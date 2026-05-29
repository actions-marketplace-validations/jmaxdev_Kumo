use anyhow::Result;
use resolver::Lockfile;

#[derive(clap::Args)]
pub struct PatchCommand {
    pub name: String,
}

#[async_trait::async_trait(?Send)]
impl super::Command for PatchCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.name.clone()).await
    }
}

pub async fn execute(name: String) -> Result<()> {
    println!("Patching package: {}...", name);
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if !lock_path.exists() {
        anyhow::bail!("kumo.lock not found.");
    }
    let lockfile: Lockfile = serde_yml::from_str(&std::fs::read_to_string(lock_path)?)?;

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
        let deps_dir = crate::common::get_deps_dir();
        let src_dir = std::env::current_dir()?
            .join(deps_dir)
            .join(&name.replace('/', std::path::MAIN_SEPARATOR_STR));
        if src_dir.exists() {
            println!("Extracting package for patching to {:?}...", patch_dir);
            crate::common::copy_dir_recursive(&src_dir, &patch_dir).await?;
            println!("Done. Package ready for modification at {:?}", patch_dir);
            println!(
                "After editing, you can use 'kumo install' to sync changes (experimental)."
            );
        }
    }
    Ok(())
}
