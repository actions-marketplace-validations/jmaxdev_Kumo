use anyhow::Result;
use resolver::Lockfile;
use security::SecurityEngine;

pub async fn execute(security: &SecurityEngine) -> Result<()> {
    println!("Scanning for fixable vulnerabilities...");
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found. Please run 'kumo install' first.");
    }

    let lockfile: Lockfile = serde_yml::from_str(&std::fs::read_to_string(&lock_path)?)?;
    let mut fixable = Vec::new();

    let pb = indicatif::ProgressBar::new(lockfile.packages.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} auditing {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    for (key, _pkg) in &lockfile.packages {
        let (name, version) = crate::common::parse_package_id(key);
        pb.set_message(format!("{}@{}", name, version));

        let vulns = security.check_vulnerabilities(&name, &version).await?;
        if !vulns.is_empty() {
            fixable.push((name.clone(), version.clone(), vulns));
        }
        pb.inc(1);
    }
    pb.finish_and_clear();

    if fixable.is_empty() {
        println!("No vulnerabilities found. Your project is clean!");
        return Ok(());
    }

    println!(
        "\nFound {} vulnerable packages:",
        fixable.len()
    );

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

    let resolver = resolver::Resolver::new();
    let mut fixed_count = 0;

    for (name, version, vulns) in &fixable {
        println!(
            "\n  \x1b[31m✗\x1b[0m {}@{} ({} {})",
            name,
            version,
            vulns.len(),
            if vulns.len() == 1 { "vulnerability" } else { "vulnerabilities" }
        );
        for v in vulns {
            println!("    - [{}] {}: {}", v.severity, v.id, v.summary);
        }

        match resolver.get_latest_version(name).await {
            Ok(latest) => {
                if &latest != version {
                    let latest_vulns = security.check_vulnerabilities(name, &latest).await?;
                    if latest_vulns.is_empty() {
                        println!(
                            "    \x1b[32m→ Fix available:\x1b[0m upgrade to {}@{}",
                            name, latest
                        );

                        let sections = ["dependencies", "devDependencies"];
                        for section in &sections {
                            if let Some(deps) = config_content.get_mut(*section).and_then(|v| v.as_object_mut()) {
                                if deps.contains_key(name) {
                                    deps.insert(name.clone(), serde_json::json!(format!("^{}", latest)));
                                    fixed_count += 1;
                                }
                            }
                        }
                    } else {
                        println!(
                            "    \x1b[33m⚠ Latest version {}@{} is also vulnerable.\x1b[0m",
                            name, latest
                        );
                    }
                } else {
                    println!("    \x1b[33m⚠ Already on latest version. No fix available.\x1b[0m");
                }
            }
            Err(_) => {
                println!("    \x1b[33m⚠ Could not resolve latest version.\x1b[0m");
            }
        }
    }

    if fixed_count > 0 {
        let json = serde_json::to_string_pretty(&config_content)?;
        std::fs::write(&config_path, json)?;
        println!(
            "\n\x1b[32m✓ Updated {} package(s) in {}.\x1b[0m",
            fixed_count,
            config_path.file_name().unwrap().to_string_lossy()
        );
        println!("Run 'kumo install' to apply the changes.");
    } else {
        println!("\nNo automatic fixes could be applied.");
    }

    Ok(())
}
