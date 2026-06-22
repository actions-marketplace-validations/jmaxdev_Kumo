use anyhow::Result;
use kumo_core::Store;
use kumo_core::shield::ShieldManager;
use resolver::{Lockfile, Resolver};
use security::SecurityEngine;
use std::collections::HashMap;
use futures::StreamExt;
use crate::common;

#[derive(clap::Args)]
pub struct InstallCommand {
    #[arg(long)]
    pub log: bool,
}

#[async_trait::async_trait(?Send)]
impl super::Command for InstallCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        let config_path = ctx.config_path.clone().ok_or_else(|| anyhow::anyhow!("Neither kumo.json nor package.json found in current directory"))?;
        execute(&ctx.store, &ctx.resolver, &ctx.security, self.log, config_path).await
    }
}

pub async fn execute(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    log: bool,
    config_path: std::path::PathBuf,
) -> Result<()> {
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

    resolve_and_install(store, resolver, security, deps, log, config_path).await?;
    Ok(())
}

pub async fn resolve_and_install(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    deps: HashMap<String, String>,
    show_logs: bool,
    config_path: std::path::PathBuf,
) -> Result<()> {
    let deps_dir_name = common::get_deps_dir();

    if !std::path::Path::new(&deps_dir_name).exists() {
        let gitignore_path = std::env::current_dir()?.join(".gitignore");
        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            let ignore_entry = format!("{}/", deps_dir_name);
            if !content
                .lines()
                .any(|l| l.trim() == deps_dir_name || l.trim() == ignore_entry)
            {
                println!("Adding {} to .gitignore", ignore_entry);
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&gitignore_path)?;
                use std::io::Write;
                if !content.ends_with('\n') && !content.is_empty() {
                    writeln!(file)?;
                }
                writeln!(file, "{}", ignore_entry)?;
            }
        }
    }

    let current_config_content = std::fs::read_to_string(&config_path)?;
    let config_hash = blake3::hash(current_config_content.as_bytes()).to_string();

    let lock_path = std::env::current_dir()?.join("kumo.lock");
    let lockfile_exists = lock_path.exists();
    let lockfile: Option<Lockfile> = if lockfile_exists {
        serde_yml::from_str(&std::fs::read_to_string(&lock_path)?).ok()
    } else {
        None
    };

    let lockfile_old_copy = lockfile.clone();

    let lockfile = if let Some(lf) = lockfile {
        if lf.config_hash == Some(config_hash.clone()) {
            println!("Configuration unchanged. Using persistent resolution cache.");
            lf
        } else {
            println!("Resolving full dependency tree...");
            let mut lf = resolver.resolve_tree(&deps).await?;
            validate_lockfile_trust(security, &lockfile_old_copy, &lf)?;
            validate_typosquatting(security, &lockfile_old_copy, &deps, &lf)?;
            lf.config_hash = Some(config_hash);
            let yaml = serde_yml::to_string(&lf)?;
            
            let shield = ShieldManager::new();
            if lock_path.exists() {
                let _ = shield.unshield_file(&lock_path);
            }
            std::fs::write(&lock_path, yaml)?;
            lf
        }
    } else {
        println!("Resolving full dependency tree...");
        let mut lf = resolver.resolve_tree(&deps).await?;
        validate_lockfile_trust(security, &lockfile_old_copy, &lf)?;
        validate_typosquatting(security, &lockfile_old_copy, &deps, &lf)?;
        lf.config_hash = Some(config_hash);
        let yaml = serde_yml::to_string(&lf)?;
        
        let shield = ShieldManager::new();
        if lock_path.exists() {
            let _ = shield.unshield_file(&lock_path);
        }
        std::fs::write(&lock_path, yaml)?;
        lf
    };

    security.validate_lockfile(&lockfile)?;

    use rayon::prelude::*;

    let mut packages_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for key in lockfile.packages.keys() {
        let (name, _) = common::parse_package_id(key);
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
                    let (_, version) = common::parse_package_id(key);
                    let (_, best_version) = common::parse_package_id(&best_key);

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

    let check_futures = winners.values().map(|key| {
        let store = store.clone();
        let key = key.clone();
        async move {
            store.load_index(&key).await.map_or(false, |opt| opt.is_some())
        }
    });
    let cached_results = futures::future::join_all(check_futures).await;
    let cached_count = cached_results.into_iter().filter(|&b| b).count();
    let download_count = winners.len() - cached_count;
    if download_count > 0 {
        println!(
            "Installing {} unique packages ({} from cache, {} to download)...",
            winners.len(),
            cached_count,
            download_count
        );
    } else {
        println!(
            "Installing {} unique packages (all {} resolved from cache)...",
            winners.len(),
            cached_count
        );
    }

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
            let (pkg_name, version) = common::parse_package_id(&key);
            let name = pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR);

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
                                    create_shim(&bin_dir, &cmd_name, &target_dir.join(p)).await?;
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
                .validate_package(&pkg_name, &version, None, false, None, has_scripts)
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
            let bytes = response.bytes().await?;

            if let Err(e) = kumo_core::tarball::verify_shasum(&bytes, &pkg.resolution.shasum) {
                if let Some(ref pb) = pb {
                    pb.finish_with_message(format!("Integrity check failed: {}", name));
                }
                return Err(e);
            }

            let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;

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
                                create_shim(&bin_dir, &cmd_name, &target_dir.join(p)).await?;
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

    let results: Vec<_> = futures::StreamExt::collect(futures::StreamExt::buffer_unordered(stream, concurrent_limit)).await;

    let mut errors = 0;
    for res in &results {
        if let Err(e) = res {
            errors += 1;
            eprintln!("Error: {}", e);
        }
    }

    if errors > 0 {
        main_pb.finish_with_message(format!("Finished with {} errors.", errors));
        anyhow::bail!("Installation failed with {} errors", errors);
    } else {
        main_pb.finish_with_message("Done!");
    }
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
                    let _ = run_install_scripts(&target_dir, scripts, security).await;
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

    let shield = ShieldManager::new();
    if shield.is_active() {
        let _ = shield.shield_file(&lock_path);
        let _ = shield.shield_file(&config_path);
    }

    Ok(())
}

pub async fn install_global(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    pkg_name: String,
    version_req: String,
) -> Result<()> {
    println!("Installing global package: {}@{}...", pkg_name, version_req);
    let metadata = resolver
        .clone()
        .resolve_package(pkg_name.clone(), version_req)
        .await?;

    let mut deps = HashMap::new();
    deps.insert(pkg_name.clone(), metadata.version.to_string());
    let lockfile = resolver.resolve_tree(&deps).await?;

    let global_root = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?.join(".kumo").join("global");
    let global_deps_dir = global_root.join("dependencies");
    let global_bin = global_root.join("bin");

    tokio::fs::create_dir_all(&global_bin).await?;

    println!("Downloading and linking global dependencies...");
    for (key, pkg) in &lockfile.packages {
        let (pkg_name, version) = common::parse_package_id(key);
        let normalized_name = pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR);

        let valid = security.validate_package(&pkg_name, &version, None, false, None, false).await.unwrap_or(false);
        if !valid {
            anyhow::bail!("Security policy violation for global package {}", key);
        }

        let bytes = reqwest::get(&pkg.resolution.tarball).await?.bytes().await?;
        kumo_core::tarball::verify_shasum(&bytes, &pkg.resolution.shasum)?;
        let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
        let target_dir = global_deps_dir.join(normalized_name);
        kumo_core::package::link_package(store, &target_dir, &file_map).await?;
    }

    if let Some(bin) = metadata.bin {
        match bin {
            serde_json::Value::String(path) => {
                create_shim(&global_bin, &pkg_name, &global_deps_dir.join(&pkg_name).join(path)).await?;
            }
            serde_json::Value::Object(map) => {
                for (cmd_name, path) in map {
                    if let Some(p) = path.as_str() {
                        create_shim(&global_bin, &cmd_name, &global_deps_dir.join(&pkg_name).join(p))
                            .await?;
                    }
                }
            }
            _ => {}
        }
    }

    println!("Global package {}@{} installed!", pkg_name, metadata.version);
    println!("Binaries linked in: {:?}", global_bin);
    println!("Add this directory to your PATH to use them.");
    Ok(())
}

async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    common::create_shim(bin_dir, name, target).await
}

async fn run_install_scripts(
    pkg_dir: &std::path::Path,
    scripts: &HashMap<String, String>,
    security_engine: &security::SecurityEngine,
) -> Result<()> {
    let proxy_port = security::proxy::start_proxy(security_engine.policy.allowed_domains.clone()).await.ok();
    
    for script_name in &["preinstall", "install", "postinstall"] {
        if let Some(script_content) = scripts.get(*script_name) {

            let mut all_warnings = Vec::new();
            
            if let Ok(warnings) = security::ast::analyze_script(script_content) {
                all_warnings.extend(warnings);
            }
            
            let words: Vec<&str> = script_content.split_whitespace().collect();
            for word in words {
                if word.ends_with(".js") || word.ends_with(".cjs") || word.ends_with(".mjs") || word.ends_with(".ts") {
                    let file_path = pkg_dir.join(word);
                    if file_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            if let Ok(warnings) = security::ast::analyze_script(&content) {
                                all_warnings.extend(warnings);
                            }
                        }
                    }
                }
            }

            if !all_warnings.is_empty() {
                eprintln!(
                    "🚨 \x1b[31mSecurity Violation:\x1b[0m Script '{}' in package {:?} blocked by Static Analysis!",
                    script_name, pkg_dir.file_name().unwrap_or_default()
                );
                for w in all_warnings {
                    eprintln!("  - {}", w);
                }
                continue;
            }

            let mut command = security::sandbox::SandboxRunner::create_command(pkg_dir, script_content, false, proxy_port);

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
                    let new_path = common::prepend_to_path(&bin_dir);
                    command.env("PATH", new_path);
                }
            }

            let status = security::sandbox::SandboxRunner::execute_command(&mut command)?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
    }
    Ok(())
}


fn validate_lockfile_trust(
    security: &SecurityEngine,
    old_lf: &Option<Lockfile>,
    new_lf: &Lockfile,
) -> Result<()> {
    for (key, new_pkg) in &new_lf.packages {
        let (name, _) = common::parse_package_id(key);

        let new_has_sigs = new_pkg.resolution.signatures.as_ref().map_or(false, |s| !s.is_empty()) || new_pkg.resolution.npm_signature.is_some();
        let new_has_atts = new_pkg.resolution.attestations.is_some();
        let new_trust = security.get_trust_level(new_has_sigs, new_has_atts);


        if security.policy.trust_policy == "strict" && new_trust == security::TrustLevel::Low {
            if !security.policy.trust_policy_exclude.contains(&name) {
                anyhow::bail!(
                    "Security policy violation: Strict trust policy is active, and package '{}' has no digital signatures or build provenance (TrustLevel: Low)!\n  Configure trust_policy_exclude in kumo.config.json if you want to bypass this.",
                    name
                );
            }
        }


        if let Some(ref old) = old_lf {

            let old_pkg_opt = old.packages.iter()
                .find(|(k, _)| {
                    let (k_name, _) = common::parse_package_id(k);
                    k_name == name
                })
                .map(|(_, p)| p);

            if let Some(old_pkg) = old_pkg_opt {
                let old_has_sigs = old_pkg.resolution.signatures.as_ref().map_or(false, |s| !s.is_empty()) || old_pkg.resolution.npm_signature.is_some();
                let old_has_atts = old_pkg.resolution.attestations.is_some();
                let old_trust = security.get_trust_level(old_has_sigs, old_has_atts);

                if !security.validate_trust_downgrade(&name, new_trust, old_trust, new_pkg.published_at.as_deref()) {
                    anyhow::bail!(
                        "Security policy violation: Trust level downgrade detected for package '{}'!\n  Previous: {}\n  New: {}\n  Configure trustPolicyExclude or trustPolicyIgnoreAfter in kumo.config.json if this is expected.",
                        name, old_trust, new_trust
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_typosquatting(
    security: &SecurityEngine,
    old_lf: &Option<Lockfile>,
    deps: &HashMap<String, String>,
    new_lf: &Lockfile,
) -> Result<()> {
    let mut existing_deps = std::collections::HashSet::new();
    for k in deps.keys() {
        existing_deps.insert(k.clone());
    }
    if let Some(ref old) = old_lf {
        for k in old.dependencies.keys() {
            existing_deps.insert(k.clone());
        }
    }

    for key in new_lf.packages.keys() {
        let (name, _) = common::parse_package_id(key);


        if security.policy.trusted_packages.contains(&name) || security.policy.trust_policy_exclude.contains(&name) {
            continue;
        }

        if let Some(similar_to) = security.check_typosquatting(&name, &existing_deps) {
            anyhow::bail!(
                "\n🚨 \x1b[31mSecurity Violation: Typosquatting Detected!\x1b[0m\n\
                 The package '{}' is suspiciously similar to the popular package '{}'.\n\
                 This is a common vector for supply chain attacks (e.g. TeamPCP/Shai-Hulud).\n\
                 If you are sure you want to install this, please add it to 'trusted_packages' in your kumo.config.json.",
                 name, similar_to
            );
        }
    }
    Ok(())
}
