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
        // Normalize relative path for the current OS
        let normalized_rel_path = rel_path.replace('/', std::path::MAIN_SEPARATOR_STR);
        let dest = target_dir.join(normalized_rel_path);
        let src = store.get_path(hash);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory for file link")?;
        }

        if dest.exists() {
            // Try to remove existing file to ensure we can create a new link/copy
            let _ = fs::remove_file(&dest).await;
        }

        if fs::hard_link(&src, &dest).await.is_err() {
            // Fallback to copy if hard_link fails (e.g. across different partitions)
            fs::copy(&src, &dest)
                .await
                .with_context(|| format!("Failed to copy file from {:?} to {:?}", src, dest))?;
        }
    }

    Ok(())
}
