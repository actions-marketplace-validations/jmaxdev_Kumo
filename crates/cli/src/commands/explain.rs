use anyhow::Result;
use resolver::Lockfile;

pub async fn execute(name: &str) -> Result<()> {
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
