use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::process::Command;

#[derive(Parser)]
#[command(name = "kx")]
#[command(about = "Kumo Execute: Run binaries from dependencies/.bin or node_modules/.bin", long_about = None)]
struct KxCli {
    binary: String,
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = KxCli::parse();
    let (store, security, resolver) = common::init_components().await?;

    // 1. Try local execution first
    let deps_dir_name = common::get_deps_dir();
    let bin_dir = std::env::current_dir()?.join(&deps_dir_name).join(".bin");
    
    let possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", cli.binary), format!("{}.exe", cli.binary), format!("{}.bat", cli.binary), cli.binary.clone()]
    } else {
        vec![cli.binary.clone()]
    };

    for bin_name in possible_bins {
        let bin_path = bin_dir.join(&bin_name);
        if bin_path.exists() {
            return execute_binary(&bin_path, cli.args, &bin_dir);
        }
    }

    // 2. Not found locally, ask to install
    println!("Package '{}' not found in {}/.bin", cli.binary, deps_dir_name);
    print!("Do you want to install and execute it using Kumo? (y/N): ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        let (bin_path, bin_dir) = install_and_get_bin(&store, &resolver, &security, &cli.binary).await?;
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

async fn install_and_get_bin(
    store: &kumo_core::Store,
    resolver: &resolver::Resolver,
    security: &security::SecurityEngine,
    name: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    println!("Resolving {}...", name);
    
    let mut root_deps = HashMap::new();
    root_deps.insert(name.to_string(), "latest".to_string());
    
    let lockfile = resolver.resolve_tree(&root_deps).await?;
    
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

    let kx_dir = dirs::home_dir().unwrap().join(".kumo").join("kx").join(name);
    let bin_dir = kx_dir.join(".bin");
    let nm_dir = kx_dir.join("node_modules");
    
    if !kx_dir.exists() {
        println!("Installing {} and dependencies...", name);
        tokio::fs::create_dir_all(&bin_dir).await?;
        
        for (pkg_id, pkg) in &lockfile.packages {
            let pkg_name = pkg_id.split('@').next().unwrap_or(pkg_id);
            let dest = nm_dir.join(pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR));
            
            // Download and extract using streaming
            let client = reqwest::Client::new();
            let stream = client.get(&pkg.resolution.tarball)
                .send()
                .await?
                .bytes_stream();
            
            let file_map = kumo_core::tarball::extract_streaming(store, stream).await?;
            kumo_core::package::link_package(store, &dest, &file_map).await?;
            
            // Create bins
            if let Some(bin) = &pkg.bin {
                match bin {
                    serde_json::Value::String(path) => {
                        create_shim(&bin_dir, pkg_name, &dest.join(path)).await?;
                    }
                    serde_json::Value::Object(map) => {
                        for (cmd_name, path) in map {
                            if let Some(p) = path.as_str() {
                                create_shim(&bin_dir, cmd_name, &dest.join(p)).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let bin_path = bin_dir.join(name);
    let bin_path_cmd = bin_dir.join(format!("{}.cmd", name));
    
    if bin_path_cmd.exists() {
        Ok((bin_path_cmd, bin_dir))
    } else if bin_path.exists() {
        Ok((bin_path, bin_dir))
    } else {
        // Find ANY bin in that directory if the name doesn't match exactly
        let mut entries = tokio::fs::read_dir(&bin_dir).await?;
        if let Some(entry) = entries.next_entry().await? {
            Ok((entry.path(), bin_dir))
        } else {
            anyhow::bail!("No binary found after installation of {}", name)
        }
    }
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
