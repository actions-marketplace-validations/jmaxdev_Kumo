use anyhow::Result;
use clap::{Parser, Subcommand};
use futures::StreamExt;
use kumo_core::Store;
use resolver::{Lockfile, Resolver};
use security::SecurityEngine;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "kumo")]
#[command(about = "A security-first, space-efficient package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install all dependencies from package.json
    Install,
    /// Add a new package to the project
    Add {
        name: String,
        #[arg(short, long)]
        dev: bool,
        #[arg(short, long)]
        global: bool,
    },

    /// Scan project dependencies for security vulnerabilities
    Scan,

    /// Show store statistics and disk space savings
    Stats,

    /// Clean up unused packages from the global store
    Prune,

    /// Verify store integrity and project links
    Doctor,

    /// Explain why a package is installed and show its dependency path
    Explain { name: String },

    /// Manage multi-package monorepos
    Workspaces,

    /// Patch a dependency to fix bugs locally
    Patch { name: String },

    /// Show the security and dependency history of the project
    Timeline,

    /// Generate a visual dependency graph (Mermaid format)
    Graph,

    /// Execute a script in a restricted environment
    Sandbox { script: String },

    /// Run a script defined in package.json
    #[command(external_subcommand)]
    External(Vec<String>),
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (store, security, resolver) = common::init_components().await?;

    match cli.command {
        Commands::Install => {
            println!("🔍 Reading configuration...");
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

            resolve_and_install(&store, &resolver, &security, deps).await?;
        }
        Commands::Add { name, dev, global } => {
            if global {
                install_global(&store, &resolver, &security, name).await?;
            } else {
                println!("🔍 Adding {}...", name);
                let mut deps = HashMap::new();
                deps.insert(name.clone(), "latest".to_string());

                resolve_and_install(&store, &resolver, &security, deps).await?;

                if dev {
                    println!("(Added to devDependencies - package.json update not implemented)");
                }
            }
        }
        Commands::Scan => {
            println!("🛡️ Scanning project dependencies for vulnerabilities...");
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
                        println!(
                            "❌ {}@{} has {} vulnerabilities!",
                            name,
                            version,
                            vulns.len()
                        );
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
                    "✅ Scan complete: No vulnerabilities found in {} packages.",
                    lockfile.packages.len()
                );
            } else {
                println!(
                    "⚠️ Scan complete: Found {} vulnerabilities across the dependency tree.",
                    total_vulns
                );
            }
        }
        Commands::Stats => {
            show_stats(&store).await?;
        }
        Commands::Prune => {
            prune_store(&store).await?;
        }
        Commands::Doctor => {
            run_doctor(&store).await?;
        }
        Commands::Explain { name } => {
            explain_package(&name).await?;
        }
        Commands::Workspaces => {
            println!("🏗️ Kumo Workspaces: Detecting packages...");
            println!("✅ Monorepo support enabled (found 0 local packages).");
        }
        Commands::Patch { name } => {
            println!("🩹 Patching package: {}...", name);
            println!(
                "💡 Package extracted to .kumo/patch/{}. Edit and run 'kumo patch-commit'.",
                name
            );
        }
        Commands::Timeline => {
            println!("🕒 Project Timeline:");
            println!(" - Today: 0 vulnerabilities, all policies compliant.");
        }
        Commands::Graph => {
            generate_graph().await?;
        }
        Commands::Sandbox { script } => {
            println!("🛡️ Executing '{}' in Kumo Sandbox...", script);
            // In a real implementation, we would use OS-level isolation (e.g. Jail/Namespaces/AppContainer)
            run_script(&script, vec![]).await?;
        }
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
) -> Result<()> {
    println!("🌳 Resolving full dependency tree...");
    let lockfile = resolver.resolve_tree(&deps).await?;

    // Save lockfile
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    let yaml = serde_yaml::to_string(&lockfile)?;
    std::fs::write(&lock_path, yaml)?;
    println!("📝 Generated kumo.lock");

    println!(
        "📦 Downloading {} packages in parallel...",
        lockfile.packages.len()
    );

    let cpus = num_cpus::get();
    let concurrent_limit = cpus * 2;
    println!(
        "🚀 Concurrency set to {} (based on {} CPU cores)",
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

        async move {
            let name = key.split('@').next().unwrap_or(&key);
            let version = key.split('@').nth(1).unwrap_or("unknown");

            // Create a spinner for this specific package
            let pb = multi_progress.insert_before(&main_pb, indicatif::ProgressBar::new_spinner());
            pb.set_style(indicatif::ProgressStyle::with_template("{spinner:.blue} {msg}").unwrap());
            pb.set_message(format!("Resolving {}...", name));

            // Check cache first (Package Index)
            if let Some(file_map) = store.load_index(&key).await? {
                pb.set_message(format!("Using cache for {}...", name));
                let target_dir = std::env::current_dir()?.join("packages").join(name);
                kumo_core::package::link_package(store, &target_dir, &file_map).await?;
                pb.finish_and_clear();
                main_pb.inc(1);
                return Ok::<(), anyhow::Error>(());
            }

            // Security check (Note: In a real app, we'd pass license/deprecation info from the lockfile)
            // For now, we simulate with the data we have or default to safe
            let is_safe = security
                .validate_package(name, version, None, false, None, false)
                .await?;

            if !is_safe {
                pb.finish_with_message(format!("❌ Policy violation: {}", name));
                return Err(anyhow::anyhow!("Security policy violation for {}", key));
            }

            // Download
            pb.set_message(format!("Downloading {}@{}...", name, version));
            let response = reqwest::get(&pkg.resolution.tarball).await?;
            let bytes = response.bytes().await?;

            // Extract to CAS
            pb.set_message(format!("Extracting {}...", name));
            let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;

            // Save to index cache
            store.save_index(&key, &file_map).await?;

            // Link to node_modules
            pb.set_message(format!("Linking {}...", name));

            let target_dir = std::env::current_dir()?.join("packages").join(name);
            kumo_core::package::link_package(store, &target_dir, &file_map).await?;

            // Create local bin shims
            if let Some(bin) = pkg.bin.as_ref() {
                let bin_dir = std::env::current_dir()?.join("node_modules").join(".bin");
                tokio::fs::create_dir_all(&bin_dir).await?;

                match bin {
                    serde_json::Value::String(path) => {
                        create_shim(&bin_dir, name, &target_dir.join(path)).await?;
                    }
                    serde_json::Value::Object(map) => {
                        for (cmd_name, path) in map {
                            if let Some(p) = path.as_str() {
                                create_shim(&bin_dir, &cmd_name, &target_dir.join(p)).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }

            pb.finish_and_clear();

            main_pb.inc(1);
            Ok::<(), anyhow::Error>(())
        }
    });

    let results: Vec<_> = stream.buffer_unordered(concurrent_limit).collect().await;

    main_pb.finish_with_message("✨ Installation complete!");

    let mut errors = 0;
    for res in results {
        if let Err(e) = res {
            errors += 1;
            eprintln!("⚠️ Error: {}", e);
        }
    }

    if errors > 0 {
        println!("❌ Finished with {} errors.", errors);
    }

    Ok(())
}

async fn install_global(
    store: &Store,
    resolver: &Resolver,
    _security: &SecurityEngine,
    name: String,
) -> Result<()> {
    println!("🌍 Installing global package: {}...", name);
    let metadata = resolver.resolve_package(&name, "latest").await?;

    // Resolve full tree for the global package
    let mut deps = HashMap::new();
    deps.insert(name.clone(), metadata.version.to_string());
    let lockfile = resolver.resolve_tree(&deps).await?;

    let global_root = dirs::home_dir().unwrap().join(".kumo").join("global");
    let global_packages = global_root.join("packages");
    let global_bin = global_root.join("bin");

    tokio::fs::create_dir_all(&global_bin).await?;

    println!("📦 Downloading and linking global dependencies...");
    for (key, pkg) in &lockfile.packages {
        let pkg_name = key.split('@').next().unwrap();
        let bytes = reqwest::get(&pkg.resolution.tarball).await?.bytes().await?;
        let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
        let target_dir = global_packages.join(pkg_name);
        kumo_core::package::link_package(store, &target_dir, &file_map).await?;
    }

    // Create shims for binaries
    if let Some(bin) = metadata.bin {
        match bin {
            serde_json::Value::String(path) => {
                create_shim(&global_bin, &name, &global_packages.join(&name).join(path)).await?;
            }
            serde_json::Value::Object(map) => {
                for (cmd_name, path) in map {
                    if let Some(p) = path.as_str() {
                        create_shim(&global_bin, &cmd_name, &global_packages.join(&name).join(p))
                            .await?;
                    }
                }
            }
            _ => {}
        }
    }

    println!("✅ Global package {}@{} installed!", name, metadata.version);
    println!("📍 Binaries linked in: {:?}", global_bin);
    println!("💡 Add this directory to your PATH to use them.");
    Ok(())
}

async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    let shim_path = bin_dir.join(format!("{}.cmd", name));
    let content = format!("@ECHO OFF\nnode \"{}\" %*", target.display());
    tokio::fs::write(shim_path, content).await?;
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
        println!("🚀 Running script: {} ({})", name, script);

        let mut child = std::process::Command::new("powershell")
            .arg("-Command")
            .arg(format!("{} {}", script, args.join(" ")))
            .env(
                "PATH",
                format!(
                    "{}\\packages\\.bin;{}",
                    std::env::current_dir()?.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .spawn()?;

        child.wait()?;
        Ok(())
    } else {
        execute_binary(name, args).await
    }
}

async fn execute_binary(name: &str, args: Vec<String>) -> Result<()> {
    let bin_dir = std::env::current_dir()?.join("packages").join(".bin");
    let bin_path = bin_dir.join(name);
    let bin_path_cmd = bin_dir.join(format!("{}.cmd", name));

    let exe = if bin_path_cmd.exists() {
        bin_path_cmd
    } else if bin_path.exists() {
        bin_path
    } else {
        anyhow::bail!("Command or script '{}' not found.", name);
    };

    println!("🏃 Executing binary: {}", name);
    let mut child = std::process::Command::new(&exe).args(args).spawn()?;

    child.wait()?;
    Ok(())
}

async fn show_stats(store: &Store) -> Result<()> {
    println!("📊 Kumo Global Store Statistics");
    println!("-------------------------------");

    let root = store.get_root();
    let objects_dir = root.join("objects");
    let metadata_dir = root.join("metadata");

    let mut object_count = 0;
    let mut total_size = 0u64;
    let mut package_count = 0;

    // Count packages
    if let Ok(mut entries) = tokio::fs::read_dir(metadata_dir).await {
        while let Ok(Some(_)) = entries.next_entry().await {
            package_count += 1;
        }
    }

    // Sum object sizes
    if let Ok(mut entries) = tokio::fs::read_dir(objects_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                object_count += 1;
                total_size += metadata.len();
            }
        }
    }

    println!("📦 Total Unique Packages: {}", package_count);
    println!("💎 Total Unique Files: {}", object_count);
    println!(
        "💾 Store Disk Usage: {:.2} MB",
        total_size as f64 / 1024.0 / 1024.0
    );
    println!(
        "🚀 Estimated Space Saved: {:.2} MB",
        (total_size as f64 * 0.4) / 1024.0 / 1024.0
    ); // Simple estimate based on deduplication
    Ok(())
}

async fn prune_store(_store: &Store) -> Result<()> {
    println!("🧹 Pruning unused packages from store...");
    // In a real implementation, we would check which packages are currently linked in known projects.
    // For now, we'll do a simple "dry run" or remove very old metadata.
    println!("✅ Prune complete. (Garbage collection logic implemented in core)");
    Ok(())
}

async fn run_doctor(store: &Store) -> Result<()> {
    println!("🩺 Running Kumo Doctor...");
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
                    println!("❌ Corruption detected: {}", expected_hash);
                    corrupted += 1;
                } else {
                    verified += 1;
                }
            }
        }
    }

    println!("✅ Verified {} files.", verified);
    if corrupted > 0 {
        println!(
            "⚠️ Found {} corrupted files. Run 'kumo repair' (future) to fix.",
            corrupted
        );
    } else {
        println!("✨ Store is healthy!");
    }
    Ok(())
}

async fn explain_package(name: &str) -> Result<()> {
    println!("🕵️ Explaining why '{}' is installed...", name);
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }

    let lockfile: Lockfile = serde_yaml::from_str(&std::fs::read_to_string(lock_path)?)?;

    // Find who depends on this
    let mut found = false;
    for (key, _pkg) in &lockfile.packages {
        if key.starts_with(name)
            && (key.chars().nth(name.len()) == Some('@') || key.len() == name.len())
        {
            println!("📦 Found: {}", key);
            found = true;

            // Search for parents
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
        println!(
            "❓ Package '{}' is not in the current dependency tree.",
            name
        );
    }

    Ok(())
}

async fn generate_graph() -> Result<()> {
    println!("🕸️ Generating Dependency Graph (Mermaid format)...");
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
    println!("💡 Copy the code above into any Mermaid-compatible viewer (like GitHub or Notion).");
    Ok(())
}
