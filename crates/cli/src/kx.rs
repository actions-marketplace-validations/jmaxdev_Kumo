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
    // Intercept the first Ctrl-C so that child processes (like cmd.exe) can handle it,
    // preventing the terminal from getting bugged with overlapping prompts.
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
        let current_version = env!("CARGO_PKG_VERSION");
        println!("\n\x1b[33m┌─────────────────────────────────────────────────────────┐\x1b[0m");
        println!("\x1b[33m│\x1b[0m  New version of Kumo available: \x1b[32mv{}\x1b[0m -> \x1b[32mv{}\x1b[0m       \x1b[33m│\x1b[0m", current_version, new_version);
        println!("\x1b[33m│\x1b[0m  Run \x1b[36mkumo update\x1b[0m to upgrade!                          \x1b[33m│\x1b[0m");
        println!("\x1b[33m└─────────────────────────────────────────────────────────┘\x1b[0m\n");
    }

    res
}

async fn inner_main(mut cli: KxCli, store: &kumo_core::Store, security: &security::SecurityEngine, resolver: &Resolver) -> Result<()> {
    // Intercept "create" command
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

    // 1. Try local execution first (dependencies/.bin walking up parent directories)
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

    // 2. Resolve version to check global cache (~/.kumo/kx)
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
                        // Keep as is
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
                // Update access time for GC
                let _ = filetime::set_file_mtime(&kx_dir, filetime::FileTime::now());
                return execute_binary(&bin_path, cli.args, &global_bin_dir);
            }
        }
    }

    // 3. Not found anywhere, ask to install
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
    
    // Set PATH and NODE_PATH
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    let new_path = std::env::join_paths(paths)?;
    
    command.env("PATH", new_path);
    
    // Add parent of bin_dir/../node_modules to NODE_PATH
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
    // Security scan
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

    if !kx_dir.exists() {
        println!("Installing {} and dependencies...", name);
        tokio::fs::create_dir_all(&bin_dir).await?;
        
        for (pkg_id, pkg) in &lockfile.packages {
            let pkg_name = if pkg_id.starts_with('@') {
                let parts: Vec<&str> = pkg_id.split('@').collect();
                if parts.len() > 1 {
                    format!("@{}", parts[1])
                } else {
                    pkg_id.to_string()
                }
            } else {
                pkg_id.split('@').next().unwrap_or(pkg_id).to_string()
            };
            let dest = nm_dir.join(pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR));
            
            // Download and extract
            let client = reqwest::Client::new();
            let response = client.get(&pkg.resolution.tarball).send().await?;
            let bytes = response.bytes().await?;
            
            let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
            kumo_core::package::link_package(store, &dest, &file_map).await?;
            
            // Create shims if it has binaries
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
        }
    }

    // Try to find the binary path in the newly installed package
    let possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", exec_bin_name), format!("{}.exe", exec_bin_name), format!("{}.bat", exec_bin_name), exec_bin_name.to_string()]
    } else {
        vec![exec_bin_name.to_string()]
    };

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
    if cfg!(target_os = "windows") {
        let shim_path = bin_dir.join(format!("{}.cmd", name));
        let content = format!("@ECHO OFF\nnode \"{}\" %*", target.display());
        tokio::fs::write(shim_path, content).await?;
    } else {
        let shim_path = bin_dir.join(name);
        let content = format!("#!/bin/sh\nnode \"{}\" \"$@\"", target.display());
        tokio::fs::write(&shim_path, content).await?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&shim_path, perms)?;
        }
    }
    Ok(())
}
