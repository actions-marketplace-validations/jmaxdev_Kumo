use anyhow::Result;
use crate::common;

#[derive(clap::Args)]
pub struct UnlinkCommand {
    pub name: String,
}

#[async_trait::async_trait(?Send)]
impl super::Command for UnlinkCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.name.clone()).await
    }
}

pub async fn execute(pkg_name: String) -> Result<()> {
    let deps_dir = common::get_deps_dir();
    let link_target = std::env::current_dir()?.join(&deps_dir).join(pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR));

    if !link_target.exists() {
        anyhow::bail!("Package '{}' is not linked (or does not exist in {}).", pkg_name, deps_dir);
    }

    let meta = std::fs::symlink_metadata(&link_target)?;
    if meta.file_type().is_symlink() {
        std::fs::remove_file(&link_target)?;
        println!("Unlinked package '{}'.", pkg_name);
        println!("Note: If you need the original package back, run `kumo install`.");
    } else {
        anyhow::bail!(
            "Package '{}' is not a symlink. If you want to remove it entirely, use 'kumo remove {}'",
            pkg_name,
            pkg_name
        );
    }

    Ok(())
}
