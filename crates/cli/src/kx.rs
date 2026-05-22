use anyhow::{anyhow, Result};
use clap::Parser;
use std::collections::HashMap;
use std::process::Command;

#[derive(Parser)]
#[command(name = "kx")]
#[command(version)]
#[command(about = "Kumo Execute: Run binaries from dependencies/.bin or node_modules/.bin", long_about = None)]
struct KxCli {
    #[arg(long, help = "Prune cached kx packages older than 7 days")]
    prune: bool,

    #[arg(long = "full-prune", help = "When pruning, delete all packages instead of only those older than 7 days")]
    full_prune: bool,

    #[arg(required_unless_present_any = ["prune", "full_prune"])]
    binary: Option<String>,

    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            if tokio::signal::ctrl_c().await.is_ok() {
                std::process::exit(1);
            }
        }
    });

    let cli = KxCli::parse();
    if cli.prune || cli.full_prune {
        prune_kx_cache(cli.full_prune).await?;
        return Ok(());
    }

    let update_check_handle = tokio::spawn(common::check_for_new_version());
    
    let (store, security, resolver) = common::init_components().await?;
    let res = inner_main(cli, &store, &security, &resolver).await;

    if let Ok(Some(new_version)) = update_check_handle.await {
        common::print_update_banner(&new_version);
    }

    res
}

async fn inner_main(mut cli: KxCli, store: &kumo_core::Store, security: &security::SecurityEngine, resolver: &Resolver) -> Result<()> {
    let binary_arg = cli.binary.clone().unwrap();

    let (binary, target_version) = if binary_arg == "create" {
        if cli.args.is_empty() {
            anyhow::bail!("'create' requires a package name. Example: kx create vite");
        }
        let target = cli.args.remove(0);
        let (pkg_part, ver_part) = common::parse_package_arg(&target);
        
        let binary_name = if pkg_part.starts_with('@') {
            if let Some(slash_idx) = pkg_part.find('/') {
                let (scope, name) = pkg_part.split_at(slash_idx);
                let name = &name[1..];
                format!("{}/create-{}", scope, name)
            } else {
                format!("{}/create", pkg_part)
            }
        } else {
            format!("create-{}", pkg_part)
        };
        (binary_name, ver_part)
    } else {
        common::parse_package_arg(&binary_arg)
    };


    let deps_dir_name = common::get_deps_dir();
    let current_dir = std::env::current_dir()?;
    let mut bin_dirs = Vec::new();
    let mut current = Some(current_dir.as_path());
    while let Some(dir) = current {
        let bin_path = dir.join(&deps_dir_name).join(".bin");
        if bin_path.exists() {
            bin_dirs.push(bin_path);
        }
        current = dir.parent();
    }
    if bin_dirs.is_empty() {
        bin_dirs.push(current_dir.join(&deps_dir_name).join(".bin"));
    }

    let possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", binary), format!("{}.exe", binary), format!("{}.bat", binary), binary.clone()]
    } else {
        vec![binary.clone()]
    };

    for bin_dir in &bin_dirs {
        for bin_name in &possible_bins {
            let bin_path = bin_dir.join(bin_name);
            if bin_path.exists() {
                return execute_binary(&bin_path, cli.args, bin_dir);
            }
        }
    }


    let mut root_deps = HashMap::new();
    root_deps.insert(binary.clone(), target_version.clone());
    let lockfile = resolver.resolve_tree(&root_deps).await?;
    
    let main_pkg_id = lockfile.packages.keys()
        .find(|k| {
            let (k_name, _) = common::parse_package_id(k);
            k_name == binary
        })
        .ok_or_else(|| anyhow!("Could not find package {} in resolution", binary))?;
    let (_, version) = common::parse_package_id(main_pkg_id);

    let mut exec_bin_name = binary.clone();
    if let Some(pkg) = lockfile.packages.get(main_pkg_id) {
        if let Some(bin) = &pkg.bin {
            match bin {
                serde_json::Value::String(_) => {
                    if exec_bin_name.contains('/') {
                        exec_bin_name = exec_bin_name.split('/').last().unwrap().to_string();
                    }
                }
                serde_json::Value::Object(map) => {
                    let un_scoped = if exec_bin_name.contains('/') {
                        exec_bin_name.split('/').last().unwrap().to_string()
                    } else {
                        exec_bin_name.clone()
                    };

                    if map.contains_key(&un_scoped) {
                        exec_bin_name = un_scoped;
                    } else if map.contains_key(&exec_bin_name) {

                    } else if let Some(first_key) = map.keys().next() {
                        if map.len() == 1 {
                            exec_bin_name = first_key.to_string();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let kx_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?.join(".kumo").join("kx").join(format!("{}@{}", binary, version));
    let global_bin_dir = kx_dir.join(".bin");
    
    let exec_possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", exec_bin_name), format!("{}.exe", exec_bin_name), format!("{}.bat", exec_bin_name), exec_bin_name.clone()]
    } else {
        vec![exec_bin_name.clone()]
    };

    if kx_dir.exists() {
        for bin_name in &exec_possible_bins {
            let bin_path = global_bin_dir.join(bin_name);
            if bin_path.exists() {
                let _ = filetime::set_file_mtime(&kx_dir, filetime::FileTime::now());
                return execute_binary(&bin_path, cli.args, &global_bin_dir);
            }
        }
        let _ = tokio::fs::remove_dir_all(&kx_dir).await;
    }


    println!("Package '{}' not found in cache.", binary);
    print!("Do you want to install and execute it using Kumo? (y/N): ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        let (bin_path, bin_dir) = install_and_get_bin_with_lockfile(store, resolver, security, &binary, &exec_bin_name, &lockfile, &kx_dir).await?;
        execute_binary(&bin_path, cli.args, &bin_dir)
    } else {
        anyhow::bail!("Execution cancelled.");
    }
}

fn execute_binary(path: &std::path::Path, args: Vec<String>, bin_dir: &std::path::Path) -> Result<()> {
    let mut command = if path.extension().and_then(|s| s.to_str()) == Some("cmd") || path.extension().and_then(|s| s.to_str()) == Some("bat") {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(path);
        c
    } else {
        Command::new(path)
    };

    command.args(args);
    

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    let new_path = std::env::join_paths(paths)?;
    
    command.env("PATH", new_path);
    

    if let Some(parent) = bin_dir.parent() {
        let nm_path = parent.join("node_modules");
        if nm_path.exists() {
            command.env("NODE_PATH", nm_path);
        }
    }

    let mut child = command.spawn()?;
    let status = child.wait()?;
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    
    Ok(())
}

use resolver::{Lockfile, Resolver};

async fn install_and_get_bin_with_lockfile(
    store: &kumo_core::Store,
    _resolver: &Resolver,
    security: &security::SecurityEngine,
    name: &str,
    exec_bin_name: &str,
    lockfile: &Lockfile,
    kx_dir: &std::path::PathBuf,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {

    println!("Scanning for vulnerabilities...");
    for (pkg_name, pkg) in &lockfile.packages {
        let version = pkg.resolution.tarball.split('/').last().unwrap_or("");
        let vulns = security.check_vulnerabilities(pkg_name, version).await?;
        if !vulns.is_empty() {
            println!("Warning: Vulnerabilities found in {}:", pkg_name);
            for v in vulns {
                println!("  [{}] {}", v.severity, v.summary);
            }
        }
    }

    let bin_dir = kx_dir.join(".bin");
    let nm_dir = kx_dir.join("node_modules");

    let possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", exec_bin_name), format!("{}.exe", exec_bin_name), format!("{}.bat", exec_bin_name), exec_bin_name.to_string()]
    } else {
        vec![exec_bin_name.to_string()]
    };

    let mut bin_exists = false;
    for bin_name in &possible_bins {
        if bin_dir.join(bin_name).exists() {
            bin_exists = true;
            break;
        }
    }

    if !bin_exists {
        println!("Installing {} and dependencies...", name);
        let _ = tokio::fs::remove_dir_all(&kx_dir).await;
        tokio::fs::create_dir_all(&bin_dir).await?;
        
        let cpus = num_cpus::get();
        let concurrent_limit = cpus * 2;

        let exec_version = lockfile.dependencies.get(name).map(|v| v.as_str()).unwrap_or("latest");
        let exec_pkg_key = format!("{}@{}", name, exec_version);
        let exec_deps = lockfile.packages.get(&exec_pkg_key).and_then(|p| p.dependencies.as_ref());

        let winners: HashMap<String, String> = lockfile
            .packages
            .keys()
            .fold(HashMap::new(), |mut acc, key| {
                let (name, version) = common::parse_package_id(key);
                
                let is_better = if let Some(existing_key) = acc.get(&name) {
                    let (_, existing_version) = common::parse_package_id(existing_key);
                    
                    let mut this_matches = false;
                    let mut existing_matches = false;
                    if let Some(deps) = exec_deps {
                        if let Some(range) = deps.get(&name) {
                            if let Ok(req) = semver::VersionReq::parse(range) {
                                if let Ok(v) = semver::Version::parse(&version) {
                                    this_matches = req.matches(&v);
                                }
                                if let Ok(ev) = semver::Version::parse(&existing_version) {
                                    existing_matches = req.matches(&ev);
                                }
                            }
                        }
                    }

                    if this_matches && !existing_matches {
                        true
                    } else if !this_matches && existing_matches {
                        false
                    } else {
                        if let (Ok(v1), Ok(v2)) = (semver::Version::parse(&version), semver::Version::parse(&existing_version)) {
                            v1 > v2
                        } else {
                            version > existing_version
                        }
                    }
                } else {
                    true
                };

                if is_better {
                    acc.insert(name, key.clone());
                }
                acc
            });

        let packages_to_install: Vec<(String, resolver::LockedPackage)> = winners
            .values()
            .filter_map(|key| lockfile.packages.get(key).map(|p| (key.clone(), p.clone())))
            .collect();

        let client = reqwest::Client::new();
        
        use futures::StreamExt;
        let stream = futures::stream::iter(packages_to_install).map(|(pkg_id, pkg)| {
            let store = store.clone();
            let nm_dir = nm_dir.clone();
            let bin_dir = bin_dir.clone();
            let client = client.clone();

            async move {
                let (pkg_name, _) = common::parse_package_id(&pkg_id);
                let dest = nm_dir.join(pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR));
                
                let response = client.get(&pkg.resolution.tarball).send().await?;
                let bytes = response.bytes().await?;
                
                let file_map = kumo_core::tarball::extract_and_store(&store, &bytes).await?;
                kumo_core::package::link_package(&store, &dest, &file_map).await?;
                
                if let Some(bin) = &pkg.bin {
                    match bin {
                        serde_json::Value::String(path) => {
                            let shim_name = if pkg_name.contains('/') {
                                pkg_name.split('/').last().unwrap_or(&pkg_name)
                            } else {
                                &pkg_name
                            };
                            create_shim(&bin_dir, shim_name, &dest.join(path)).await?;
                        }
                        serde_json::Value::Object(map) => {
                            for (cmd_name, path) in map {
                                if let Some(p) = path.as_str() {
                                    create_shim(&bin_dir, &cmd_name, &dest.join(p)).await?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                Ok::<(), anyhow::Error>(())
            }
        }).buffer_unordered(concurrent_limit);

        let mut results = stream;
        while let Some(res) = results.next().await {
            res?;
        }
    }

    for bin_name in possible_bins {
        let bin_path = bin_dir.join(bin_name);
        if bin_path.exists() {
            return Ok((bin_path, bin_dir));
        }
    }

    anyhow::bail!("Binary '{}' not found after installation", name)
}
async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    common::create_shim(bin_dir, name, target).await
}

async fn prune_kx_cache(full: bool) -> Result<()> {
    let kx_root = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kumo")
        .join("kx");
    if !kx_root.exists() {
        println!("KX cache is already empty.");
        return Ok(());
    }

    if full {
        println!("Performing FULL prune of KX cache...");
        tokio::fs::remove_dir_all(&kx_root).await?;
        tokio::fs::create_dir_all(&kx_root).await?;
        println!("KX cache cleared.");
    } else {
        println!("Pruning old KX packages (older than 7 days)...");
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&kx_root).await?;
        let now = std::time::SystemTime::now();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    let accessed = metadata.accessed().unwrap_or_else(|_| {
                        metadata.modified().unwrap_or(now)
                    });
                    if now.duration_since(accessed).map(|d| d.as_secs() > 7 * 24 * 3600).unwrap_or(false) {
                        let _ = tokio::fs::remove_dir_all(&path).await;
                        count += 1;
                    }
                }
            }
        }
        println!("Removed {} old KX packages.", count);
    }
    Ok(())
}
