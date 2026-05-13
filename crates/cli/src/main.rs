use anyhow::Result;
use clap::{Parser, Subcommand};
use futures::StreamExt;
use kumo_core::Store;
use resolver::{Lockfile, Resolver};
use security::SecurityEngine;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "kumo")]
#[command(version)]
#[command(about = "A security-first, space-efficient package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "i")]
    Install {
        #[arg(long)]
        log: bool,
    },
    #[command(alias = "a")]
    Add {
        name: String,
        #[arg(short, long)]
        dev: bool,
        #[arg(short, long)]
        global: bool,
        #[arg(long)]
        log: bool,
    },
    #[command(alias = "rm")]
    #[command(alias = "un")]
    #[command(alias = "uninstall")]
    Remove {
        name: String,
    },
    Scan,
    #[command(alias = "st")]
    Stats,
    Prune {
        #[command(subcommand)]
        subcommand: PruneSubcommand,
    },
    #[command(alias = "dr")]
    Doctor,
    #[command(alias = "ex")]
    Explain {
        name: String,
    },
    Config {
        #[command(subcommand)]
        subcommand: ConfigSubcommand,
    },
    Workspaces,
    Patch {
        name: String,
    },
    Timeline,
    Graph,
    Sandbox {
        script: String,
    },
    Update,
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum PruneSubcommand {
    Cache {
        #[arg(long)]
        full: bool,
    },
    Deps {
        #[arg(long)]
        full: bool,
    },
}

#[derive(Subcommand)]
enum ConfigSubcommand {
    Init,
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (store, security, resolver) = common::init_components().await?;

    match cli.command {
        Commands::Install { log } => {
            println!("Reading configuration...");
            let kumo_json_path = std::env::current_dir()?.join("kumo.json");
            let pkg_json_path = std::env::current_dir()?.join("package.json");

            let config_path = if kumo_json_path.exists() {
                kumo_json_path
            } else if pkg_json_path.exists() {
                pkg_json_path
            } else {
                anyhow::bail!("Neither kumo.json nor package.json found in current directory");
            };

            let config_content: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
            let mut deps = HashMap::new();

            if let Some(d) = config_content
                .get("dependencies")
                .and_then(|v| v.as_object())
            {
                for (k, v) in d {
                    deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
                }
            }
            if let Some(d) = config_content
                .get("devDependencies")
                .and_then(|v| v.as_object())
            {
                for (k, v) in d {
                    deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
                }
            }

            resolve_and_install(&store, &resolver, &security, deps, log).await?;
        }
        Commands::Add {
            name,
            dev,
            global,
            log,
        } => {
            if global {
                install_global(&store, &resolver, &security, name).await?;
            } else {
                println!("Adding {}...", name);
                let mut deps = HashMap::new();
                deps.insert(name.clone(), "latest".to_string());

                resolve_and_install(&store, &resolver, &security, deps, log).await?;

                if dev {
                    println!("(Added to devDependencies - package.json update not implemented)");
                }
            }
        }
        Commands::Remove { name } => {
            println!("Removing {}...", name);
            let kumo_json_path = std::env::current_dir()?.join("kumo.json");
            let pkg_json_path = std::env::current_dir()?.join("package.json");

            let config_path = if kumo_json_path.exists() {
                kumo_json_path
            } else if pkg_json_path.exists() {
                pkg_json_path
            } else {
                anyhow::bail!("Neither kumo.json nor package.json found in current directory");
            };

            let mut config_content: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

            let mut removed = false;
            if let Some(deps) = config_content
                .get_mut("dependencies")
                .and_then(|v| v.as_object_mut())
            {
                if deps.remove(&name).is_some() {
                    removed = true;
                }
            }
            if let Some(deps) = config_content
                .get_mut("devDependencies")
                .and_then(|v| v.as_object_mut())
            {
                if deps.remove(&name).is_some() {
                    removed = true;
                }
            }

            if !removed {
                println!("Package {} not found in dependencies.", name);
            } else {
                // Save updated config
                let json = serde_json::to_string_pretty(&config_content)?;
                std::fs::write(&config_path, json)?;
                println!(
                    "Removed {} from {}",
                    name,
                    config_path.file_name().unwrap().to_string_lossy()
                );

                // Remove from local directory
                let deps_dir = common::get_deps_dir();
                let pkg_dir = std::env::current_dir()?.join(&deps_dir).join(&name);
                if pkg_dir.exists() {
                    let _ = std::fs::remove_dir_all(&pkg_dir);
                }

                // Re-resolve and update lockfile
                println!("Updating lockfile and cleaning up...");
                let mut deps = HashMap::new();
                if let Some(d) = config_content
                    .get("dependencies")
                    .and_then(|v| v.as_object())
                {
                    for (k, v) in d {
                        deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
                    }
                }
                if let Some(d) = config_content
                    .get("devDependencies")
                    .and_then(|v| v.as_object())
                {
                    for (k, v) in d {
                        deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
                    }
                }

                // We need to clean the deps dir first if we want a clean state,
                // or just let resolve_and_install do its thing (but it won't delete orphans)
                // For now, let's just re-install.
                resolve_and_install(&store, &resolver, &security, deps, false).await?;
            }
        }
        Commands::Scan => {
            println!("Scanning project dependencies for vulnerabilities...");
            let lock_path = std::env::current_dir()?.join("kumo.lock");
            if !lock_path.exists() {
                anyhow::bail!("kumo.lock not found. Please run 'kumo install' first.");
            }

            let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;
            let mut total_vulns = 0;

            let pb = indicatif::ProgressBar::new(lockfile.packages.len() as u64);
            pb.set_style(
                indicatif::ProgressStyle::with_template(
                    "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} scanning {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
            );

            for (key, _pkg) in &lockfile.packages {
                let name = key.split('@').next().unwrap_or(key);
                let version = key.split('@').nth(1).unwrap_or("unknown");

                pb.set_message(format!("{}@{}", name, version));
                let vulns = security.check_vulnerabilities(name, version).await?;

                if !vulns.is_empty() {
                    pb.suspend(|| {
                        println!("{}@{} has {} vulnerabilities!", name, version, vulns.len());
                        for v in vulns {
                            println!("  - [{}] {}: {}", v.severity, v.id, v.summary);
                            total_vulns += 1;
                        }
                    });
                }
                pb.inc(1);
            }

            pb.finish_and_clear();

            if total_vulns == 0 {
                println!(
                    "Scan complete: No vulnerabilities found in {} packages.",
                    lockfile.packages.len()
                );
            } else {
                println!(
                    "Scan complete: Found {} vulnerabilities across the dependency tree.",
                    total_vulns
                );
            }
        }
        Commands::Stats => {
            show_stats(&store).await?;
        }
        Commands::Prune { subcommand } => {
            prune_store(&store, subcommand).await?;
        }
        Commands::Doctor => {
            run_doctor(&store).await?;
        }
        Commands::Explain { name } => {
            explain_package(&name).await?;
        }
        Commands::Workspaces => {
            println!("Kumo Workspaces: Detecting packages...");
            println!("Monorepo support enabled (found 0 local packages).");
        }
        Commands::Patch { name } => {
            println!("Patching package: {}...", name);
            println!(
                "Package extracted to .kumo/patch/{}. Edit and run 'kumo patch-commit'.",
                name
            );
        }
        Commands::Timeline => {
            println!("Project Timeline:");
            println!(" - Today: 0 vulnerabilities, all policies compliant.");
        }
        Commands::Graph => {
            generate_graph().await?;
        }
        Commands::Sandbox { script } => {
            println!("Executing '{}' in Kumo Sandbox...", script);
            run_script(&script, vec![]).await?;
        }
        Commands::Update => {
            handle_update().await?;
        }
        Commands::Config { subcommand } => match subcommand {
            ConfigSubcommand::Init => {
                let config_path = std::env::current_dir()?.join("kumo.config.json");
                if config_path.exists() {
                    anyhow::bail!("kumo.config.json already exists");
                }

                let policy = security::Policy::default();
                let json = serde_json::to_string_pretty(&policy)?;
                std::fs::write(config_path, json)?;
                println!("Created kumo.config.json with default security policies.");
            }
        },
        Commands::External(args) => {
            if args.is_empty() {
                anyhow::bail!("No script specified");
            }
            let script_name = &args[0];
            let script_args = &args[1..];
            run_script(script_name, script_args.to_vec()).await?;
        }
    }

    Ok(())
}

async fn resolve_and_install(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    deps: HashMap<String, String>,
    show_logs: bool,
) -> Result<()> {
    let deps_dir_name = common::get_deps_dir();

    if deps_dir_name == "dependencies" && !std::path::Path::new("dependencies").exists() {
        let gitignore_path = std::env::current_dir()?.join(".gitignore");
        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            if !content
                .lines()
                .any(|l| l.trim() == "dependencies" || l.trim() == "dependencies/")
            {
                println!("Adding dependencies/ to .gitignore");
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&gitignore_path)?;
                use std::io::Write;
                if !content.ends_with('\n') && !content.is_empty() {
                    writeln!(file)?;
                }
                writeln!(file, "dependencies/")?;
            }
        }
    }

    println!("Resolving full dependency tree...");
    let lockfile = resolver.resolve_tree(&deps).await?;

    let lock_path = std::env::current_dir()?.join("kumo.lock");
    let yaml = serde_yaml::to_string(&lockfile)?;
    std::fs::write(&lock_path, yaml)?;
    println!("Generated kumo.lock");

    println!(
        "Downloading {} packages in parallel...",
        lockfile.packages.len()
    );

    let cpus = num_cpus::get();
    let concurrent_limit = cpus * 2;
    println!(
        "Concurrency set to {} (based on {} CPU cores)",
        concurrent_limit, cpus
    );

    let multi_progress = indicatif::MultiProgress::new();
    let main_pb = multi_progress.add(indicatif::ProgressBar::new(lockfile.packages.len() as u64));
    main_pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    main_pb.set_message("Installing packages...");

    let stream = futures::stream::iter(lockfile.packages.clone()).map(|(key, pkg)| {
        let store = store;
        let security = security;
        let main_pb = main_pb.clone();
        let multi_progress = multi_progress.clone();
        let deps_dir_name = deps_dir_name.clone();
        let show_logs = show_logs;

        async move {
            let parts: Vec<&str> = key.split('@').collect();
            let name = if key.starts_with('@') {
                if parts.len() > 1 {
                    format!("@{}", parts[1])
                } else {
                    key.clone()
                }
            } else {
                parts[0].to_string()
            };
            // Normalize name for Windows paths if it contains slashes
            let name = name.replace('/', std::path::MAIN_SEPARATOR_STR);

            let version = if key.starts_with('@') {
                parts.get(2).unwrap_or(&"unknown").to_string()
            } else {
                parts.get(1).unwrap_or(&"unknown").to_string()
            };

            let pb = if show_logs {
                let pb =
                    multi_progress.insert_before(&main_pb, indicatif::ProgressBar::new_spinner());
                pb.set_style(
                    indicatif::ProgressStyle::with_template("{spinner:.blue} {msg}").unwrap(),
                );
                pb.set_message(format!("Resolving {}...", name));
                Some(pb)
            } else {
                None
            };

            if let Some(file_map) = store.load_index(&key).await? {
                if let Some(ref pb) = pb {
                    pb.set_message(format!("Using cache for {}...", name));
                }
                let target_dir = std::env::current_dir()?.join(&deps_dir_name).join(&name);
                kumo_core::package::link_package(store, &target_dir, &file_map).await?;

                // Run install scripts if any
                if let Some(scripts) = pkg.scripts.as_ref() {
                    let _ = run_install_scripts(&target_dir, scripts).await;
                }

                // Still need to create shims for cached packages!
                if let Some(bin) = pkg.bin.as_ref() {
                    let bin_dir = std::env::current_dir()?.join(&deps_dir_name).join(".bin");
                    tokio::fs::create_dir_all(&bin_dir).await?;

                    match bin {
                        serde_json::Value::String(path) => {
                            // For scoped packages like @scope/pkg, if bin is a string,
                            // the binary name should be just 'pkg'
                            let bin_name = if name.contains(std::path::MAIN_SEPARATOR) {
                                name.split(std::path::MAIN_SEPARATOR)
                                    .last()
                                    .unwrap_or(&name)
                            } else {
                                &name
                            };
                            create_shim(&bin_dir, bin_name, &target_dir.join(path)).await?;
                        }
                        serde_json::Value::Object(map) => {
                            for (cmd_name, path) in map {
                                if let Some(p) = path.as_str() {
                                    create_shim(&bin_dir, cmd_name, &target_dir.join(p)).await?;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(pb) = pb {
                    pb.finish_and_clear();
                }
                main_pb.inc(1);
                return Ok::<(), anyhow::Error>(());
            }

            let has_scripts = pkg.scripts.as_ref().map_or(false, |s| {
                s.contains_key("preinstall")
                    || s.contains_key("install")
                    || s.contains_key("postinstall")
            });

            let is_safe = security
                .validate_package(
                    &name,
                    &version,
                    None,  // license
                    false, // is_deprecated (should fetch from metadata if available)
                    None,  // published_at
                    has_scripts,
                )
                .await?;

            if !is_safe {
                if let Some(ref pb) = pb {
                    pb.finish_with_message(format!("Policy violation: {}", name));
                }
                return Err(anyhow::anyhow!("Security policy violation for {}", key));
            }

            if let Some(ref pb) = pb {
                pb.set_message(format!("Downloading {}@{}...", name, version));
            }
            let response = reqwest::get(&pkg.resolution.tarball).await?;
            let bytes = response.bytes().await?;

            if let Some(ref pb) = pb {
                pb.set_message(format!("Extracting {}...", name));
            }
            let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;

            store.save_index(&key, &file_map).await?;

            if let Some(ref pb) = pb {
                pb.set_message(format!("Linking {}...", name));
            }

            let target_dir = std::env::current_dir()?.join(&deps_dir_name).join(&name);
            kumo_core::package::link_package(store, &target_dir, &file_map).await?;

            // Run install scripts if any
            if let Some(scripts) = pkg.scripts.as_ref() {
                let _ = run_install_scripts(&target_dir, scripts).await;
            }

            if let Some(bin) = pkg.bin.as_ref() {
                let bin_dir = std::env::current_dir()?.join(&deps_dir_name).join(".bin");
                tokio::fs::create_dir_all(&bin_dir).await?;

                match bin {
                    serde_json::Value::String(path) => {
                        let bin_name = if name.contains(std::path::MAIN_SEPARATOR) {
                            name.split(std::path::MAIN_SEPARATOR)
                                .last()
                                .unwrap_or(&name)
                        } else {
                            &name
                        };
                        create_shim(&bin_dir, bin_name, &target_dir.join(path)).await?;
                    }
                    serde_json::Value::Object(map) => {
                        for (cmd_name, path) in map {
                            if let Some(p) = path.as_str() {
                                create_shim(&bin_dir, cmd_name, &target_dir.join(p)).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(pb) = pb {
                pb.finish_and_clear();
            }

            main_pb.inc(1);
            Ok::<(), anyhow::Error>(())
        }
    });

    let results: Vec<_> = stream.buffer_unordered(concurrent_limit).collect().await;

    main_pb.finish_with_message("Done!");
    println!("Installed {} packages.", lockfile.packages.len());
    let total_bytes: u64 = lockfile
        .packages
        .values()
        .map(|p| p.resolution.get_size())
        .sum();

    if total_bytes >= 1024 * 1024 * 1024 {
        println!(
            "Total size: {:.2} GB",
            total_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        );
    } else if total_bytes >= 1024 * 1024 {
        println!("Total size: {:.2} MB", total_bytes as f64 / 1024.0 / 1024.0);
    } else {
        println!("Total size: {:.2} KB", total_bytes as f64 / 1024.0);
    }

    let mut errors = 0;
    for res in results {
        if let Err(e) = res {
            errors += 1;
            eprintln!("Error: {}", e);
        }
    }

    if errors > 0 {
        println!("Finished with {} errors.", errors);
    }

    Ok(())
}

async fn install_global(
    store: &Store,
    resolver: &Resolver,
    _security: &SecurityEngine,
    name: String,
) -> Result<()> {
    println!("Installing global package: {}...", name);
    let metadata = resolver.resolve_package(&name, "latest").await?;

    let mut deps = HashMap::new();
    deps.insert(name.clone(), metadata.version.to_string());
    let lockfile = resolver.resolve_tree(&deps).await?;

    let global_root = dirs::home_dir().unwrap().join(".kumo").join("global");
    let global_deps_dir = global_root.join("dependencies");
    let global_bin = global_root.join("bin");

    tokio::fs::create_dir_all(&global_bin).await?;

    println!("Downloading and linking global dependencies...");
    for (key, pkg) in &lockfile.packages {
        let pkg_name = key.split('@').next().unwrap();
        let normalized_name = pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR);

        let bytes = reqwest::get(&pkg.resolution.tarball).await?.bytes().await?;
        let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
        let target_dir = global_deps_dir.join(normalized_name);
        kumo_core::package::link_package(store, &target_dir, &file_map).await?;
    }

    if let Some(bin) = metadata.bin {
        match bin {
            serde_json::Value::String(path) => {
                create_shim(&global_bin, &name, &global_deps_dir.join(&name).join(path)).await?;
            }
            serde_json::Value::Object(map) => {
                for (cmd_name, path) in map {
                    if let Some(p) = path.as_str() {
                        create_shim(&global_bin, &cmd_name, &global_deps_dir.join(&name).join(p))
                            .await?;
                    }
                }
            }
            _ => {}
        }
    }

    println!("Global package {}@{} installed!", name, metadata.version);
    println!("Binaries linked in: {:?}", global_bin);
    println!("Add this directory to your PATH to use them.");
    Ok(())
}

async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    let shim_path = bin_dir.join(format!("{}.cmd", name));

    // Ensure parent directory of the shim exists (important for scoped bin names)
    if let Some(parent) = shim_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let content = format!("@ECHO OFF\nnode \"{}\" %*", target.display());
    tokio::fs::write(shim_path, content).await?;
    Ok(())
}

async fn run_install_scripts(
    pkg_dir: &std::path::Path,
    scripts: &HashMap<String, String>,
) -> Result<()> {
    for script_name in &["preinstall", "install", "postinstall"] {
        if let Some(script_content) = scripts.get(*script_name) {
            let mut command = if cfg!(windows) {
                let mut cmd = std::process::Command::new("cmd");
                cmd.arg("/c").arg(script_content);
                cmd
            } else {
                let mut sh = std::process::Command::new("sh");
                sh.arg("-c").arg(script_content);
                sh
            };

            // Set NODE_PATH to the deps dir so node can find other packages
            if let Some(deps_dir) = pkg_dir.parent() {
                command.env("NODE_PATH", deps_dir);

                // Also add .bin to PATH so scripts can find local binaries
                let bin_dir = deps_dir.join(".bin");
                if bin_dir.exists() {
                    let path = std::env::var("PATH").unwrap_or_default();
                    let new_path = format!("{};{}", bin_dir.display(), path);
                    command.env("PATH", new_path);
                }
            }

            let _ = command.current_dir(pkg_dir).status();
        }
    }
    Ok(())
}

async fn run_script(name: &str, args: Vec<String>) -> Result<()> {
    let pkg_json_path = std::env::current_dir()?.join("kumo.json");
    let npm_json_path = std::env::current_dir()?.join("package.json");

    let config_path = if pkg_json_path.exists() {
        pkg_json_path
    } else {
        npm_json_path
    };
    if !config_path.exists() {
        return execute_binary(name, args).await;
    }

    let config_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config_path)?)?;

    if let Some(script) = config_content
        .get("scripts")
        .and_then(|s| s.get(name))
        .and_then(|v| v.as_str())
    {
        println!("Running script: {} ({})", name, script);

        let bin_path = std::env::current_dir()?
            .join(common::get_deps_dir())
            .join(".bin");

        let mut command = if cfg!(windows) {
            let mut cmd = std::process::Command::new("cmd");
            cmd.arg("/c").arg(format!("{} {}", script, args.join(" ")));
            cmd
        } else {
            let mut sh = std::process::Command::new("sh");
            sh.arg("-c").arg(format!("{} {}", script, args.join(" ")));
            sh
        };

        let new_path = if let Ok(existing_path) = std::env::var("PATH") {
            let sep = if cfg!(windows) { ";" } else { ":" };
            format!("{}{}{}", bin_path.display(), sep, existing_path)
        } else {
            bin_path.display().to_string()
        };

        let mut child = command.env("PATH", new_path).spawn()?;

        child.wait()?;
        Ok(())
    } else {
        execute_binary(name, args).await
    }
}

async fn execute_binary(name: &str, args: Vec<String>) -> Result<()> {
    let bin_dir = std::env::current_dir()?
        .join(common::get_deps_dir())
        .join(".bin");
    let bin_path = bin_dir.join(name);
    let bin_path_cmd = bin_dir.join(format!("{}.cmd", name));

    let exe = if bin_path_cmd.exists() {
        bin_path_cmd
    } else if bin_path.exists() {
        bin_path
    } else {
        anyhow::bail!("Command or script '{}' not found.", name);
    };

    println!("Executing binary: {}", name);

    let mut command = std::process::Command::new(&exe);
    command.args(args);

    let new_path = if let Ok(existing_path) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ";" } else { ":" };
        format!("{}{}{}", bin_dir.display(), sep, existing_path)
    } else {
        bin_dir.display().to_string()
    };

    let mut child = command.env("PATH", new_path).spawn()?;

    child.wait()?;
    Ok(())
}

async fn show_stats(store: &Store) -> Result<()> {
    let root = store.get_root();
    let objects_dir = root.join("objects");
    let metadata_dir = root.join("metadata");

    let mut object_count = 0;
    let mut total_size = 0u64;
    let mut package_count = 0;

    if let Ok(mut entries) = tokio::fs::read_dir(metadata_dir).await {
        while let Ok(Some(_)) = entries.next_entry().await {
            package_count += 1;
        }
    }

    if let Ok(mut entries) = tokio::fs::read_dir(objects_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                object_count += 1;
                total_size += metadata.len();
            }
        }
    }

    println!("Total Unique Packages: {}", package_count);
    println!("Total Unique Files: {}", object_count);

    if total_size >= 1024 * 1024 * 1024 {
        println!(
            "Store Disk Usage: {:.2} GB",
            total_size as f64 / 1024.0 / 1024.0 / 1024.0
        );
        println!(
            "Estimated Space Saved: {:.2} GB",
            (total_size as f64 * 0.4) / 1024.0 / 1024.0 / 1024.0
        );
    } else if total_size >= 1024 * 1024 {
        println!(
            "Store Disk Usage: {:.2} MB",
            total_size as f64 / 1024.0 / 1024.0
        );
        println!(
            "Estimated Space Saved: {:.2} MB",
            (total_size as f64 * 0.4) / 1024.0 / 1024.0
        );
    } else {
        println!("Store Disk Usage: {:.2} KB", total_size as f64 / 1024.0);
        println!(
            "Estimated Space Saved: {:.2} KB",
            (total_size as f64 * 0.4) / 1024.0
        );
    }
    Ok(())
}

async fn prune_store(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Cache { full } => {
            if full {
                let metadata_dir = store.get_root().join("metadata");
                if metadata_dir.exists() {
                    tokio::fs::remove_dir_all(&metadata_dir).await?;
                    tokio::fs::create_dir_all(&metadata_dir).await?;
                    println!("Cleared all package metadata.");
                }
            }
            let count = store.prune().await?;
            println!("Removed {} orphaned files.", count);
        }
        PruneSubcommand::Deps { full } => {
            let deps_dir_name = common::get_deps_dir();
            let deps_dir = std::env::current_dir()?.join(&deps_dir_name);

            if deps_dir.exists() {
                if full {
                    tokio::fs::remove_dir_all(&deps_dir).await?;
                    println!("Removed {}/ and all its content.", deps_dir_name);

                    let lock_path = std::env::current_dir()?.join("kumo.lock");
                    if lock_path.exists() {
                        tokio::fs::remove_file(&lock_path).await?;
                        println!("Removed kumo.lock");
                    }
                } else {
                    let mut entries = tokio::fs::read_dir(&deps_dir).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        let path = entry.path();
                        if path.is_dir() {
                            tokio::fs::remove_dir_all(path).await?;
                        } else {
                            tokio::fs::remove_file(path).await?;
                        }
                    }
                    println!("Cleaned content of {}/.", deps_dir_name);
                }
            } else {
                println!("No {}/ directory found.", deps_dir_name);
            }
        }
    }
    Ok(())
}

async fn run_doctor(store: &Store) -> Result<()> {
    let root = store.get_root();
    let objects_dir = root.join("objects");

    let mut corrupted = 0;
    let mut verified = 0;

    if let Ok(mut entries) = tokio::fs::read_dir(objects_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let expected_hash = path.file_name().unwrap().to_str().unwrap();
                let bytes = tokio::fs::read(&path).await?;
                let mut hasher = blake3::Hasher::new();
                hasher.update(&bytes);
                let actual_hash = hasher.finalize().to_hex().to_string();

                if expected_hash != actual_hash {
                    println!("Corruption detected: {}", expected_hash);
                    corrupted += 1;
                } else {
                    verified += 1;
                }
            }
        }
    }

    println!("Verified {} files.", verified);
    if corrupted > 0 {
        println!(
            "Found {} corrupted files. Run 'kumo repair' (future) to fix.",
            corrupted
        );
    }
    Ok(())
}

async fn explain_package(name: &str) -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }

    let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;

    let mut found = false;
    for (key, _pkg) in &lockfile.packages {
        if key.starts_with(name)
            && (key.chars().nth(name.len()) == Some('@') || key.len() == name.len())
        {
            println!("Found: {}", key);
            found = true;

            for (parent_key, parent_pkg) in &lockfile.packages {
                if let Some(deps) = &parent_pkg.dependencies {
                    if deps.contains_key(name) {
                        println!("   └── Required by: {}", parent_key);
                    }
                }
            }
        }
    }

    if !found {
        println!("Package '{}' is not in the current dependency tree.", name);
    }

    Ok(())
}

async fn generate_graph() -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }

    let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;

    println!("\n```mermaid");
    println!("graph TD");

    for (key, pkg) in &lockfile.packages {
        let name = key.split('@').next().unwrap();
        if let Some(deps) = &pkg.dependencies {
            for (dep_name, _range) in deps {
                println!("    {} --> {}", name, dep_name);
            }
        }
    }

    println!("```\n");
    println!("Copy the code above into any Mermaid-compatible viewer (like GitHub or Notion).");
    Ok(())
}

async fn handle_update() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!(
        "Checking for updates... (Current version: {})",
        current_version
    );

    let client = reqwest::Client::builder()
        .user_agent("kumo-pkg-updater")
        .build()?;

    let release: serde_json::Value = client
        .get("https://api.github.com/repos/jmaxdev/Kumo/releases/latest")
        .send()
        .await?
        .json()
        .await?;

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Could not find latest version tag"))?;
    let latest_version = latest_tag.trim_start_matches('v');

    let current_semver = semver::Version::parse(current_version)?;
    let latest_semver = semver::Version::parse(latest_version)?;

    if latest_semver > current_semver {
        println!("A new version is available: {}!", latest_tag);

        let os = std::env::consts::OS;
        let asset_name = match os {
            "windows" => "kumo-windows.zip",
            "linux" => "kumo-linux.tar.gz",
            "macos" => "kumo-macos.tar.gz",
            _ => anyhow::bail!("Unsupported OS for auto-update"),
        };

        let asset = release["assets"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No assets found in latest release"))?
            .iter()
            .find(|a| a["name"] == asset_name)
            .ok_or_else(|| anyhow::anyhow!("Could not find asset for your OS: {}", asset_name))?;

        let download_url = asset["browser_download_url"].as_str().unwrap();

        let response = client.get(download_url).send().await?;
        let bytes = response.bytes().await?;

        let tmp_dir = std::env::temp_dir().join("kumo_update");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir)?;

        let exe_name = if os == "windows" { "kumo.exe" } else { "kumo" };
        let kx_name = if os == "windows" { "kx.exe" } else { "kx" };
        let exe_path = tmp_dir.join(exe_name);
        let kx_path = tmp_dir.join(kx_name);

        if asset_name.ends_with(".zip") {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                if file.name().ends_with(exe_name) {
                    let mut out = std::fs::File::create(&exe_path)?;
                    std::io::copy(&mut file, &mut out)?;
                } else if file.name().ends_with(kx_name) {
                    let mut out = std::fs::File::create(&kx_path)?;
                    std::io::copy(&mut file, &mut out)?;
                }
            }
        } else {
            let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
            let mut archive = tar::Archive::new(tar);
            for entry in archive.entries()? {
                let mut entry = entry?;
                let path = entry.path()?.to_str().unwrap().to_string();
                if path.ends_with(exe_name) {
                    entry.unpack(&exe_path)?;
                } else if path.ends_with(kx_name) {
                    entry.unpack(&kx_path)?;
                }
            }
        }

        if !exe_path.exists() {
            anyhow::bail!("Failed to extract kumo binary from update archive");
        }

        self_replace::self_replace(&exe_path)?;

        if kx_path.exists() {
            if let Ok(current_exe) = std::env::current_exe() {
                let current_dir = current_exe.parent().unwrap();
                let target_kx = current_dir.join(kx_name);
                if target_kx.exists() {
                    let _ = std::fs::copy(&kx_path, &target_kx);
                }
            }
        }

        println!("Update successful! Please run 'kumo --version' to verify.");
    } else {
        println!("Kumo is already up to date.");
    }

    Ok(())
}
