use anyhow::Result;
use resolver::Lockfile;
use security::SecurityEngine;

pub async fn execute(security: &SecurityEngine) -> Result<()> {
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
    Ok(())
}
