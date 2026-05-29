use anyhow::Result;
use crate::common;

#[derive(clap::Args)]
pub struct LinkCommand {
    pub path: String,
}

#[async_trait::async_trait(?Send)]
impl super::Command for LinkCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.path.clone()).await
    }
}

pub async fn execute(path: String) -> Result<()> {
    let source = std::path::Path::new(&path);
    let source = if source.is_relative() {
        std::env::current_dir()?.join(source)
    } else {
        source.to_path_buf()
    };

    if !source.exists() {
        anyhow::bail!("Path '{}' does not exist", source.display());
    }

    let config_path = source.join("package.json");
    let kumo_path = source.join("kumo.json");
    let manifest = if config_path.exists() {
        config_path
    } else if kumo_path.exists() {
        kumo_path
    } else {
        anyhow::bail!(
            "No package.json or kumo.json found in '{}'. Cannot determine package name.",
            source.display()
        );
    };

    let content = std::fs::read_to_string(&manifest)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;
    let pkg_name = v["name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No 'name' field found in {}", manifest.display()))?;

    let deps_dir = common::get_deps_dir();
    let link_target = std::env::current_dir()?.join(&deps_dir).join(pkg_name.replace('/', std::path::MAIN_SEPARATOR_STR));

    if link_target.exists() {
        let meta = std::fs::symlink_metadata(&link_target)?;
        if meta.file_type().is_symlink() {
            let _ = std::fs::remove_file(&link_target);
        } else {
            let _ = std::fs::remove_dir_all(&link_target);
        }
    }

    if let Some(parent) = link_target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(target_os = "windows")]
    {
        std::os::windows::fs::symlink_dir(&source, &link_target)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::os::unix::fs::symlink(&source, &link_target)?;
    }

    println!(
        "Linked {} -> {}",
        pkg_name,
        source.display()
    );
    println!(
        "You can now import '{}' directly in your project.",
        pkg_name
    );
    Ok(())
}
