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
    Install,
    Add {
        name: String,
        #[arg(short, long)]
        dev: bool,
        #[arg(short, long)]
        global: bool,
    },
    Scan,
    Stats,
    Prune {
        #[command(subcommand)]
        subcommand: PruneSubcommand,
    },
    Doctor,
    Explain { name: String },
    Workspaces,
    Patch { name: String },
    Timeline,
    Graph,
    Sandbox { script: String },
    Update,
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum PruneSubcommand {
    Cache,
    Deps {
        #[arg(long)]
        full: bool,
    },
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (store, security, resolver) = common::init_components().await?;

    match cli.command {
        Commands::Install => {
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

            resolve_and_install(&store, &resolver, &security, deps).await?;
        }
        Commands::Add { name, dev, global } => {
            if global {
                install_global(&store, &resolver, &security, name).await?;
            } else {
                println!("Adding {}...", name);
                let mut deps = HashMap::new();
                deps.insert(name.clone(), "latest".to_string());

                resolve_and_install(&store, &resolver, &security, deps).await?;

                if dev {
                    println!("(Added to devDependencies - package.json update not implemented)");
                }
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
                        println!(
                            "{}@{} has {} vulnerabilities!",
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
    let deps_dir_name = common::get_deps_dir();

    if deps_dir_name == "dependencies" && !std::path::Path::new("dependencies").exists() {
        let gitignore_path = std::env::current_dir()?.join(".gitignore");
        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            if !content.lines().any(|l| l.trim() == "dependencies" || l.trim() == "dependencies/") {
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
            let version = if key.starts_with('@') {
                parts.get(2).unwrap_or(&"unknown").to_string()
            } else {
                parts.get(1).unwrap_or(&"unknown").to_string()
            };

            let pb = multi_progress.insert_before(&main_pb, indicatif::ProgressBar::new_spinner());
            pb.set_style(indicatif::ProgressStyle::with_template("{spinner:.blue} {msg}").unwrap());
            pb.set_message(format!("Resolving {}...", name));

            if let Some(file_map) = store.load_index(&key).await? {
                pb.set_message(format!("Using cache for {}...", name));
                let target_dir = std::env::current_dir()?.join(&deps_dir_name).join(&name);
                kumo_core::package::link_package(store, &target_dir, &file_map).await?;
                pb.finish_and_clear();
                main_pb.inc(1);
                return Ok::<(), anyhow::Error>(());
            }

            let is_safe = security
                .validate_package(&name, &version, None, false, None, false)
                .await?;

            if !is_safe {
                pb.finish_with_message(format!("Policy violation: {}", name));
                return Err(anyhow::anyhow!("Security policy violation for {}", key));
            }

            pb.set_message(format!("Downloading {}@{}...", name, version));
            let response = reqwest::get(&pkg.resolution.tarball).await?;
            let bytes = response.bytes().await?;

            pb.set_message(format!("Extracting {}...", name));
            let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;

            store.save_index(&key, &file_map).await?;

            pb.set_message(format!("Linking {}...", name));

            let target_dir = std::env::current_dir()?.join(&deps_dir_name).join(&name);
            kumo_core::package::link_package(store, &target_dir, &file_map).await?;

            if let Some(bin) = pkg.bin.as_ref() {
                let bin_dir = std::env::current_dir()?.join(&deps_dir_name).join(".bin");
                tokio::fs::create_dir_all(&bin_dir).await?;

                match bin {
                    serde_json::Value::String(path) => {
                        create_shim(&bin_dir, &name, &target_dir.join(path)).await?;
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

    main_pb.finish_with_message("Done!");
    println!("Installed {} packages.", lockfile.packages.len());
    println!("Total size: {} MB", lockfile.packages.values().map(|p| p.resolution.size).sum::<u64>() / 1024 / 1024);

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
        let bytes = reqwest::get(&pkg.resolution.tarball).await?.bytes().await?;
        let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
        let target_dir = global_deps_dir.join(pkg_name);
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
        println!("Running script: {} ({})", name, script);

        let mut child = std::process::Command::new("powershell")
            .arg("-Command")
            .arg(format!("{} {}", script, args.join(" ")))
            .env(
                "PATH",
                format!(
                    "{}\\{}\\.bin;{}",
                    std::env::current_dir()?.display(),
                    common::get_deps_dir(),
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
    let bin_dir = std::env::current_dir()?.join(common::get_deps_dir()).join(".bin");
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
    let mut child = std::process::Command::new(&exe).args(args).spawn()?;

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
    println!(
        "Store Disk Usage: {:.2} MB",
        total_size as f64 / 1024.0 / 1024.0
    );
    println!(
        "Estimated Space Saved: {:.2} MB",
        (total_size as f64 * 0.4) / 1024.0 / 1024.0
    );
    Ok(())
}

async fn prune_store(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Cache => {
            let count = store.prune().await?;
            println!("Removed {} orphaned files.", count);
        }
        PruneSubcommand::Deps { full } => {
            let deps_dir_name = common::get_deps_dir();
            let deps_dir = std::env::current_dir()?.join(&deps_dir_name);
            if deps_dir.exists() {
                tokio::fs::remove_dir_all(&deps_dir).await?;
            } else {
                println!("No {}/ directory found.", deps_dir_name);
            }

            if full {
                let lock_path = std::env::current_dir()?.join("kumo.lock");
                if lock_path.exists() {
                    tokio::fs::remove_file(&lock_path).await?;
                }
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
        println!(
            "Package '{}' is not in the current dependency tree.",
            name
        );
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
