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
    Update {
        #[arg(long)]
        pre: bool,
    },
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

    let kumo_json_path = std::env::current_dir()?.join("kumo.json");
    let pkg_json_path = std::env::current_dir()?.join("package.json");
    let config_path = if kumo_json_path.exists() {
        kumo_json_path
    } else if pkg_json_path.exists() {
        pkg_json_path
    } else {
        anyhow::bail!("Neither kumo.json nor package.json found in current directory");
    };

    match cli.command {
        Commands::Install { log } => {
            println!("Reading configuration...");
            let config_content: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
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

            resolve_and_install(&store, &resolver, &security, deps, log, config_path).await?;
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
                println!("Adding {} to configuration...", name);
                let mut config_content: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
                let section = if dev {
                    "devDependencies"
                } else {
                    "dependencies"
                };
                if let Some(obj) = config_content.as_object_mut() {
                    obj.entry(section.to_string())
                        .or_insert(serde_json::json!({}))
                        .as_object_mut()
                        .unwrap()
                        .insert(name.clone(), serde_json::json!("latest"));
                }
                
                let json = serde_json::to_string_pretty(&config_content)?;
                std::fs::write(&config_path, json)?;
                println!(
                    "Updated {} with {}",
                    config_path.file_name().unwrap().to_string_lossy(),
                    name
                );

                let mut deps = HashMap::new();
                deps.insert(name.clone(), "latest".to_string());
                resolve_and_install(&store, &resolver, &security, deps, log, config_path).await?;
            }
        }
        Commands::Remove { name } => {
            println!("Removing {}...", name);
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
                let json = serde_json::to_string_pretty(&config_content)?;
                std::fs::write(&config_path, json)?;
                println!(
                    "Removed {} from {}",
                    name,
                    config_path.file_name().unwrap().to_string_lossy()
                );

                let deps_dir = common::get_deps_dir();
                let pkg_dir = std::env::current_dir()?.join(&deps_dir).join(&name);
                if pkg_dir.exists() {
                    let _ = std::fs::remove_dir_all(&pkg_dir);
                }

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

                resolve_and_install(&store, &resolver, &security, deps, false, config_path).await?;
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
            println!("Kumo Workspaces: Detecting local packages...");
            let mut found = 0;
            if let Ok(entries) = std::fs::read_dir(".") {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let pkg_json = entry.path().join("package.json");
                        let kumo_json = entry.path().join("kumo.json");
                        if pkg_json.exists() || kumo_json.exists() {
                            let path = if pkg_json.exists() {
                                pkg_json
                            } else {
                                kumo_json
                            };
                            if let Ok(content) = std::fs::read_to_string(path) {
                                let v: serde_json::Value =
                                    serde_json::from_str(&content).unwrap_or_default();
                                let name = v["name"].as_str().unwrap_or("unknown");
                                let version = v["version"].as_str().unwrap_or("0.0.0");
                                println!(" - {} (v{}) at {:?}", name, version, entry.path());
                                found += 1;
                            }
                        }
                    }
                }
            }
            if found == 0 {
                println!("No local workspaces found. Kumo supports monorepos with package.json/kumo.json in subdirectories.");
            } else {
                println!("Found {} local packages.", found);
            }
        }
        Commands::Patch { name } => {
            println!("Patching package: {}...", name);
            let lock_path = std::env::current_dir()?.join("kumo.lock");
            if !lock_path.exists() {
                anyhow::bail!("kumo.lock not found.");
            }
            let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;

            let mut pkg_key = None;
            for key in lockfile.packages.keys() {
                if key.starts_with(&name) {
                    pkg_key = Some(key.clone());
                    break;
                }
            }

            if let Some(_key) = pkg_key {
                let patch_dir = std::env::current_dir()?
                    .join(".kumo")
                    .join("patch")
                    .join(&name);
                std::fs::create_dir_all(&patch_dir)?;
                let deps_dir = common::get_deps_dir();
                let src_dir = std::env::current_dir()?
                    .join(deps_dir)
                    .join(&name.replace('/', std::path::MAIN_SEPARATOR_STR));
                if src_dir.exists() {
                    println!("Extracting package for patching to {:?}...", patch_dir);
                    copy_dir_recursive(&src_dir, &patch_dir).await?;
                    println!("Done. Package ready for modification at {:?}", patch_dir);
                    println!(
                        "After editing, you can use 'kumo install' to sync changes (experimental)."
                    );
                }
            }
        }
        Commands::Timeline => {
            let lock_path = std::env::current_dir()?.join("kumo.lock");
            if let Ok(metadata) = std::fs::metadata(&lock_path) {
                let created = metadata.created().unwrap_or(metadata.modified().unwrap());
                let modified = metadata.modified().unwrap();
                println!("Project Timeline (based on kumo.lock):");
                println!(" - Created: {:?}", created);
                println!(" - Last Update: {:?}", modified);
                if let Ok(lockfile_str) = std::fs::read_to_string(&lock_path) {
                    if let Ok(lockfile) = serde_yaml::from_str::<Lockfile>(&lockfile_str) {
                        println!(" - Dependencies: {}", lockfile.packages.len());
                    }
                }
            } else {
                println!("No timeline available. Run 'kumo install' to generate a lockfile.");
            }
        }
        Commands::Graph => {
            generate_graph().await?;
        }
        Commands::Sandbox { script } => {
            println!("Executing '{}' in Kumo Sandbox...", script);
            run_script(&script, vec![]).await?;
        }
        Commands::Update { pre } => {
            handle_update(pre).await?;
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
    config_path: std::path::PathBuf,
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

    let current_config_content = std::fs::read_to_string(&config_path)?;
    let config_hash = blake3::hash(current_config_content.as_bytes()).to_string();

    let lock_path = std::env::current_dir()?.join("kumo.lock");
    let lockfile_exists = lock_path.exists();
    let lockfile: Option<Lockfile> = if lockfile_exists {
        serde_yaml::from_str(&std::fs::read_to_string(&lock_path)?).ok()
    } else {
        None
    };

    let lockfile = if let Some(lf) = lockfile {
        if lf.config_hash == Some(config_hash.clone()) {
            println!("Configuration unchanged. Using persistent resolution cache.");
            lf
        } else {
            println!("Resolving full dependency tree...");
            let mut lf = resolver.resolve_tree(&deps).await?;
            lf.config_hash = Some(config_hash);
            let yaml = serde_yaml::to_string(&lf)?;
            std::fs::write(&lock_path, yaml)?;
            lf
        }
    } else {
        println!("Resolving full dependency tree...");
        let mut lf = resolver.resolve_tree(&deps).await?;
        lf.config_hash = Some(config_hash);
        let yaml = serde_yaml::to_string(&lf)?;
        std::fs::write(&lock_path, yaml)?;
        lf
    };

    use rayon::prelude::*;

    let mut packages_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for key in lockfile.packages.keys() {
        let parts: Vec<&str> = key.split('@').collect();
        let name = if key.starts_with('@') {
            format!("@{}", parts[1])
        } else {
            parts[0].to_string()
        };
        packages_by_name.entry(name).or_default().push(key.clone());
    }

    let winners: HashMap<String, String> = packages_by_name
        .into_par_iter()
        .map(|(name, versions)| {
            let mut best_key = versions[0].clone();

            if let Some(root_version) = lockfile.dependencies.get(&name) {
                let root_key = format!("{}@{}", name, root_version);
                if versions.contains(&root_key) {
                    best_key = root_key;
                }
            } else {
                for key in &versions {
                    let parts: Vec<&str> = key.split('@').collect();
                    let version = if key.starts_with('@') {
                        parts.get(2).unwrap_or(&"0.0.0").to_string()
                    } else {
                        parts.get(1).unwrap_or(&"0.0.0").to_string()
                    };

                    let best_parts: Vec<&str> = best_key.split('@').collect();
                    let best_version = if best_key.starts_with('@') {
                        best_parts.get(2).unwrap_or(&"0.0.0").to_string()
                    } else {
                        best_parts.get(1).unwrap_or(&"0.0.0").to_string()
                    };

                    let v_new = semver::Version::parse(&version)
                        .unwrap_or_else(|_| semver::Version::new(0, 0, 0));
                    let v_old = semver::Version::parse(&best_version)
                        .unwrap_or_else(|_| semver::Version::new(0, 0, 0));
                    if v_new > v_old {
                        best_key = key.clone();
                    }
                }
            }
            (name, best_key)
        })
        .collect();

    println!(
        "Downloading and linking {} unique packages...",
        winners.len()
    );

    let cpus = num_cpus::get();
    let concurrent_limit = cpus * 2;

    let multi_progress = indicatif::MultiProgress::new();
    let main_pb = multi_progress.add(indicatif::ProgressBar::new(winners.len() as u64));
    main_pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    main_pb.set_message("Installing packages...");

    let packages_to_install: Vec<(String, resolver::LockedPackage)> = winners
        .values()
        .filter_map(|key| lockfile.packages.get(key).map(|p| (key.clone(), p.clone())))
        .collect();

    let stream = futures::stream::iter(packages_to_install).map(|(key, pkg)| {
        let store = store;
        let security = security;
        let resolver = resolver.clone();
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
                return Ok::<(), anyhow::Error>(());
            }

            let has_scripts = pkg.scripts.as_ref().map_or(false, |s| {
                s.contains_key("preinstall")
                    || s.contains_key("install")
                    || s.contains_key("postinstall")
            });

            let is_safe = security
                .validate_package(&name, &version, None, false, None, has_scripts)
                .await?;

            if !is_safe {
                if let Some(ref pb) = pb {
                    pb.finish_with_message(format!("Policy violation: {}", name));
                }
                return Err(anyhow::anyhow!("Security policy violation for {}", key));
            }

            if let Some(ref pb) = pb {
                pb.set_message(format!("Streaming {}@{}...", name, version));
            }
            let response = resolver
                .client()
                .get(&pkg.resolution.tarball)
                .send()
                .await?;
            let stream = response.bytes_stream();

            let file_map = kumo_core::tarball::extract_streaming(store, stream).await?;

            store.save_index(&key, &file_map).await?;

            if let Some(ref pb) = pb {
                pb.set_message(format!("Linking {}...", name));
            }

            let target_dir = std::env::current_dir()?.join(&deps_dir_name).join(&name);
            kumo_core::package::link_package(store, &target_dir, &file_map).await?;

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
    println!("Installed {} unique packages.", winners.len());

    println!("Running lifecycle scripts...");
    for (name, key) in &winners {
        if let Some(pkg) = lockfile.packages.get(key) {
            if let Some(scripts) = &pkg.scripts {
                let normalized_name = name.replace('/', std::path::MAIN_SEPARATOR_STR);
                let target_dir = std::env::current_dir()?
                    .join(&deps_dir_name)
                    .join(&normalized_name);
                if !scripts.is_empty() {
                    let _ = run_install_scripts(&target_dir, scripts).await;
                }
            }
        }
    }

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
    let metadata = resolver
        .clone()
        .resolve_package(name.clone(), "latest".to_string())
        .await?;

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

    if let Some(parent) = shim_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let deps_dir = bin_dir.parent().unwrap_or(bin_dir);
    let content = format!(
        "@ECHO OFF\nSET NODE_PATH={}\nnode \"{}\" %*",
        deps_dir.display(),
        target.display()
    );
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

            if let Some(deps_dir) = pkg_dir.parent() {
                let real_deps_dir = if deps_dir
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map_or(false, |s| s.starts_with('@'))
                {
                    deps_dir.parent().unwrap_or(deps_dir)
                } else {
                    deps_dir
                };

                command.env("NODE_PATH", real_deps_dir);
                command.env("NODE_NO_WARNINGS", "1");

                let bin_dir = deps_dir.join(".bin");
                if bin_dir.exists() {
                    let path = std::env::var("PATH").unwrap_or_default();
                    let new_path = format!("{};{}", bin_dir.display(), path);
                    command.env("PATH", new_path);
                }
            }

            let status = command.current_dir(pkg_dir).status()?;
            if !status.success() {
                eprintln!("Warning: Script '{}' failed for {:?}", script_name, pkg_dir);
            }
        }
    }
    Ok(())
}

async fn show_stats(store: &Store) -> Result<()> {
    let root = store.get_root();
    let objects_dir = root.join("objects");
    let mut total_size = 0;
    let mut file_count = 0;

    if objects_dir.exists() {
        let mut entries = tokio::fs::read_dir(objects_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let mut files = tokio::fs::read_dir(entry.path()).await?;
                while let Some(file) = files.next_entry().await? {
                    total_size += file.metadata().await?.len();
                    file_count += 1;
                }
            }
        }
    }

    println!("Kumo Global Store Stats:");
    println!("Location: {:?}", root);
    println!("Total objects: {}", file_count);
    println!("Total size: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
    Ok(())
}

async fn prune_store(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Cache { full } => {
            if full {
                println!("Performing FULL prune of global store...");
                let root = store.get_root();
                let metadata_dir = root.join("metadata");
                let objects_dir = root.join("objects");
                
                if metadata_dir.exists() {
                    let _ = std::fs::remove_dir_all(&metadata_dir);
                    let _ = std::fs::create_dir_all(&metadata_dir);
                }
                if objects_dir.exists() {
                    let _ = std::fs::remove_dir_all(&objects_dir);
                    let _ = std::fs::create_dir_all(&objects_dir);
                }
                println!("Global store cleared.");
            } else {
                println!("Pruning unreferenced global store objects...");
                let deleted = store.prune().await?;
                println!("Cleaned up {} unreferenced objects.", deleted);
            }
        }
        PruneSubcommand::Deps { full } => {
            let deps_dir = common::get_deps_dir();
            println!("Pruning {} directory...", deps_dir);
            if std::path::Path::new(&deps_dir).exists() {
                std::fs::remove_dir_all(&deps_dir)?;
                println!("Deleted local {} directory.", deps_dir);
            }
            if full {
                let lock_path = std::env::current_dir()?.join("kumo.lock");
                if lock_path.exists() {
                    std::fs::remove_file(lock_path)?;
                    println!("Deleted kumo.lock");
                }
            }
        }
    }
    Ok(())
}

async fn run_doctor(store: &Store) -> Result<()> {
    println!("Kumo Doctor: Checking system health...");
    let root = store.get_root();
    if root.exists() {
        println!("[OK] Global store exists at {:?}", root);
    } else {
        println!("[WARN] Global store not found. Running init...");
        store.init().await?;
    }

    let node_version = std::process::Command::new("node").arg("--version").output();
    match node_version {
        Ok(output) => println!(
            "[OK] Node.js found: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(_) => println!("[ERROR] Node.js not found in PATH"),
    }

    println!("Health check complete.");
    Ok(())
}

async fn explain_package(name: &str) -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }

    let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;
    let mut found = false;

    for (key, pkg) in &lockfile.packages {
        if key.starts_with(name)
            && (key.len() == name.len() || key.chars().nth(name.len()) == Some('@'))
        {
            println!("Package: {}", key);
            let parts: Vec<&str> = key.split('@').collect();
            let pkg_name = if key.starts_with('@') {
                format!("@{}", parts[1])
            } else {
                parts[0].to_string()
            };
            if lockfile.dependencies.contains_key(&pkg_name) {
                println!("Reason: Direct dependency in configuration.");
            } else {
                println!("Reason: Transient dependency (required by another package).");
            }
            if let Some(deps) = &pkg.dependencies {
                println!("Dependencies: {} packages", deps.len());
                for (d_name, d_range) in deps {
                    println!("  - {} ({})", d_name, d_range);
                }
            }
            found = true;
        }
    }

    if !found {
        println!("Package '{}' not found in current lockfile.", name);
    }
    Ok(())
}

async fn generate_graph() -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }
    let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;

    let mut dot = String::from("digraph G {\n");
    dot.push_str("  node [shape=box, fontname=\"Arial\"];\n");

    for (name, version) in &lockfile.dependencies {
        dot.push_str(&format!("  \"Project\" -> \"{}@{}\";\n", name, version));
    }

    for (key, pkg) in &lockfile.packages {
        if let Some(deps) = &pkg.dependencies {
            for (d_name, d_range) in deps {
                let mut d_key = format!("{}@{}", d_name, d_range);
                for k in lockfile.packages.keys() {
                    if k.starts_with(d_name) {
                        d_key = k.clone();
                        break;
                    }
                }
                dot.push_str(&format!("  \"{}\" -> \"{}\";\n", key, d_key));
            }
        }
    }

    dot.push_str("}\n");
    std::fs::write("dependency-graph.dot", dot)?;
    println!("Graph saved to dependency-graph.dot. Use 'dot -Tsvg dependency-graph.dot -o graph.svg' to visualize.");
    Ok(())
}

async fn run_script(name: &str, args: Vec<String>) -> Result<()> {
    // 1. Try to find script in package.json or kumo.json
    let project_dir = std::env::current_dir()?;
    let config_files = ["package.json", "kumo.json"];
    
    for config_file in config_files {
        let path = project_dir.join(config_file);
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            if let Some(script_cmd) = v["scripts"][name].as_str() {
                println!("> {}", script_cmd);
                let mut shell_cmd = if cfg!(target_os = "windows") {
                    let mut c = std::process::Command::new("cmd");
                    c.arg("/c").arg(script_cmd);
                    c
                } else {
                    let mut c = std::process::Command::new("sh");
                    c.arg("-c").arg(script_cmd);
                    c
                };
                
                // Add dependencies/.bin to PATH so scripts can find installed tools
                let deps_dir = common::get_deps_dir();
                let bin_dir = project_dir.join(deps_dir).join(".bin");
                if let Some(old_path) = std::env::var_os("PATH") {
                    let mut paths = std::vec![bin_dir];
                    paths.extend(std::env::split_paths(&old_path));
                    let new_path = std::env::join_paths(paths)?;
                    shell_cmd.env("PATH", new_path);
                }

                shell_cmd.args(args);
                let status = shell_cmd.status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
                return Ok(());
            }
        }
    }

    // 2. Fallback: Try to find binary in .bin
    let deps_dir = common::get_deps_dir();
    let bin_dir = project_dir.join(&deps_dir).join(".bin");
    
    let possible_bins = if cfg!(target_os = "windows") {
        vec![format!("{}.cmd", name), format!("{}.exe", name), format!("{}.bat", name), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    for bin_name in possible_bins {
        let bin_path = bin_dir.join(&bin_name);
        if bin_path.exists() {
            let mut command = if bin_name.ends_with(".cmd") || bin_name.ends_with(".bat") {
                let mut c = std::process::Command::new("cmd");
                c.arg("/c").arg(bin_path);
                c
            } else {
                std::process::Command::new(bin_path)
            };
            
            command.args(args);
            let status = command.status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
            return Ok(());
        }
    }

    anyhow::bail!("Script or binary '{}' not found in configuration or .bin", name);
}

async fn handle_update(include_pre: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);
    println!("Checking for updates...");

    let client = reqwest::Client::builder()
        .user_agent("kumo-pkg-manager")
        .build()?;

    let url = if include_pre {
        "https://api.github.com/repos/jmaxdev/kumo/releases"
    } else {
        "https://api.github.com/repos/jmaxdev/kumo/releases/latest"
    };

    let response = client.get(url).send().await?;
    
    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::NOT_FOUND && !include_pre {
            anyhow::bail!("No stable release found. Try 'kumo update --pre' to check for alpha/beta versions.");
        }
        anyhow::bail!("GitHub API error ({}). Please try again later.", response.status());
    }

    let release_val: serde_json::Value = response.json().await?;
    let release: serde_json::Value = if include_pre {
        if let Some(arr) = release_val.as_array() {
            arr.first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("No releases found in repository."))?
        } else {
            release_val
        }
    } else {
        release_val
    };

    if let Some(msg) = release.get("message").and_then(|m| m.as_str()) {
        anyhow::bail!("GitHub API Error: {}", msg);
    }

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| {
            if !include_pre {
                anyhow::anyhow!("No stable release found. Use 'kumo update --pre' for latest development versions.")
            } else {
                anyhow::anyhow!("Could not find version information in the latest release.")
            }
        })?;
    let latest_version = latest_tag.trim_start_matches('v');

    if latest_version == current_version {
        println!("Kumo is already up to date!");
        return Ok(());
    }

    println!(
        "A new version is available: v{} -> v{}",
        current_version, latest_version
    );

    #[cfg(target_os = "windows")]
    let asset_name = "kumo-windows.zip";
    #[cfg(target_os = "macos")]
    let asset_name = "kumo-macos.tar.gz";
    #[cfg(target_os = "linux")]
    let asset_name = "kumo-linux.tar.gz";

    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets found in release"))?;
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str().unwrap_or("").contains(asset_name))
        .ok_or_else(|| anyhow::anyhow!("Could not find asset for current OS: {}", asset_name))?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Asset download URL missing"))?;

    println!("Downloading update from {}...", download_url);
    let response = client.get(download_url).send().await?;
    let bytes = response.bytes().await?;

    let temp_dir = std::env::temp_dir().join("kumo_update");
    std::fs::create_dir_all(&temp_dir)?;
    
    let bin_path = if asset_name.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        let target_name = if archive.file_names().any(|n| n == "kumo.exe") {
            "kumo.exe"
        } else {
            "kumo"
        };
        let mut file = archive.by_name(target_name)?;
        let out_path = temp_dir.join(file.name());
        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out_file)?;
        out_path
    } else {
        let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
        let mut archive = tar::Archive::new(tar);
        let mut bin_path = None;
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            if path.file_name().and_then(|s| s.to_str()) == Some("kumo") {
                let out_path = temp_dir.join("kumo");
                entry.unpack(&out_path)?;
                bin_path = Some(out_path);
                break;
            }
        }
        bin_path.ok_or_else(|| anyhow::anyhow!("Binary 'kumo' not found in archive"))?
    };

    println!("Applying update...");
    self_replace::self_replace(&bin_path)?;
    
    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("Successfully updated to v{}!", latest_version);
    Ok(())
}

async fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(&dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            Box::pin(copy_dir_recursive(
                &entry.path(),
                &dst.join(entry.file_name()),
            ))
            .await?;
        } else {
            tokio::fs::copy(entry.path(), dst.join(entry.file_name())).await?;
        }
    }
    Ok(())
}
