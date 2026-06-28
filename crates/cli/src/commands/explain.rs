use anyhow::Result;
use resolver::Lockfile;

#[derive(clap::Args)]
pub struct ExplainCommand {
    pub name: String,
}

#[async_trait::async_trait(?Send)]
impl super::Command for ExplainCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(&self.name).await
    }
}

pub async fn execute(name: &str) -> Result<()> {
    let lock_path = std::env::current_dir()?.join(kumo_core::config::KUMO_LOCK);
    if !lock_path.exists() {
        anyhow::bail!("{} not found.", kumo_core::config::KUMO_LOCK);
    }

    let lockfile: Lockfile = serde_yml::from_str(&std::fs::read_to_string(lock_path)?)?;
    let mut found = false;

    for (key, pkg) in &lockfile.packages {
        if key.starts_with(name)
            && (key.len() == name.len() || key.chars().nth(name.len()) == Some('@'))
        {
            println!("Package: {}", key);
            let (pkg_name, _) = crate::common::parse_package_id(key);
            if lockfile.dependencies.contains_key(&pkg_name) {
                println!("Reason: Direct dependency in configuration.");
            } else {
                println!("Reason: Transitive dependency (required by another package).");
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
