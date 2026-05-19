use anyhow::{anyhow, Result};
use clap::Parser;
use std::collections::HashMap;
use std::process::Command;

#[derive(Parser)]
#[command(name = "kx")]
#[command(version)]
#[command(about = "Kumo Execute: Run binaries from dependencies/.bin or node_modules/.bin", long_about = None)]
struct KxCli {
    binary: String,
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
    let update_check_handle = tokio::spawn(common::check_for_new_version());
    
    let (store, security, resolver) = common::init_components().await?;
    let res = inner_main(cli, &store, &security, &resolver).await;

    if let Ok(Some(new_version)) = update_check_handle.await {
        common::print_update_banner(&new_version);
    }

    res
}

async fn inner_main(mut cli: KxCli, store: &kumo_core::Store, security: &security::SecurityEngine, resolver: &Resolver) -> Result<()> {

    if cli.binary == "create" {
        if cli.args.is_empty() {
            anyhow::bail!("'create' requires a package name. Example: kx create vite");
        }
        let target = cli.args.remove(0);
        
        if target.starts_with('@') {
            if let Some(slash_idx) = target.find('/') {
                let (scope, name) = target.split_at(slash_idx);
                let name = &name[1..];
                cli.binary = format!("{}/create-{}", scope, name);
            } else {
                cli.binary = format!("{}/create", target);
            }
        } else {
            cli.binary = format!("create-{}", target);
        }
    }


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
        vec![format!("{}.cmd", cli.binary), format!("{}.exe", cli.binary), format!("{}.bat", cli.binary), cli.binary.clone()]
    } else {
        vec![cli.binary.clone()]
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
    root_deps.insert(cli.binary.clone(), "latest".to_string());
    let lockfile = resolver.resolve_tree(&root_deps).await?;
    
    let main_pkg_id = lockfile.packages.keys()
        .find(|k| k.starts_with(&cli.binary))
        .ok_or_else(|| anyhow!("Could not find package {} in resolution", cli.binary))?;
    let version = main_pkg_id.split('@').last().unwrap_or("latest");

    let mut exec_bin_name = cli.binary.clone();
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

    let kx_dir = dirs::home_dir().unwrap().join(".kumo").join("kx").join(format!("{}@{}", cli.binary, version));
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


    println!("Package '{}' not found in cache.", cli.binary);
    print!("Do you want to install and execute it using Kumo? (y/N): ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        let (bin_path, bin_dir) = install_and_get_bin_with_lockfile(store, resolver, security, &cli.binary, &exec_bin_name, &lockfile, &kx_dir).await?;
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

        let packages_to_install: Vec<(String, resolver::LockedPackage)> = lockfile.packages
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let client = reqwest::Client::new();
        
        use futures::StreamExt;
        let stream = futures::stream::iter(packages_to_install).map(|(pkg_id, pkg)| {
            let store = store.clone();
            let nm_dir = nm_dir.clone();
            let bin_dir = bin_dir.clone();
            let client = client.clone();

            async move {
                let pkg_name = if pkg_id.starts_with('@') {
                    let parts: Vec<&str> = pkg_id.split('@').collect();
                    if parts.len() > 1 {
                        format!("@{}", parts[1])
                    } else {
                        pkg_id.to_string()
                    }
                } else {
                    pkg_id.split('@').next().unwrap_or(&pkg_id).to_string()
                };
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
