use crate::Store;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

pub async fn link_package(
    store: &Store,
    target_dir: &Path,
    file_map: &HashMap<String, String>,
) -> Result<()> {
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)
            .await
            .context("Failed to create target directory")?;
    }

    for (rel_path, hash) in file_map {
        let dest = target_dir.join(rel_path);
        let src = store.get_path(hash);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory for file link")?;
        }

        if dest.exists() {
            fs::remove_file(&dest).await.ok();
        }

        fs::hard_link(&src, &dest)
            .await
            .with_context(|| format!("Failed to create hardlink from {:?} to {:?}", src, dest))?;
    }

    Ok(())
}
