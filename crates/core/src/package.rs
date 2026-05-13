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

    let mut tasks = futures_util::stream::FuturesUnordered::new();

    for (rel_path, hash) in file_map {
        let store = store.clone();
        let target_dir = target_dir.to_path_buf();
        let rel_path = rel_path.clone();
        let hash = hash.clone();

        tasks.push(async move {
            let normalized_rel_path = rel_path.replace('/', std::path::MAIN_SEPARATOR_STR);
            let dest = target_dir.join(normalized_rel_path);
            let src = store.get_path(&hash);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create parent directory for file link")?;
            }

            if dest.exists() {
                let _ = fs::remove_file(&dest).await;
            }

            let src_clone = src.clone();
            let dest_clone = dest.clone();

            let result = tokio::task::spawn_blocking(move || {
                if reflink_copy::reflink(&src_clone, &dest_clone).is_ok() {
                    return Ok(());
                }
                if std::fs::hard_link(&src_clone, &dest_clone).is_ok() {
                    return Ok(());
                }
                match std::fs::copy(&src_clone, &dest_clone) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow::anyhow!("IO Error: {} (Source: {:?}, Dest: {:?})", e, src_clone, dest_clone)),
                }
            })
            .await?;

            result.with_context(|| format!("Failed to link/copy package file"))
        });
    }

    use futures_util::StreamExt;
    while let Some(res) = tasks.next().await {
        res?;
    }

    Ok(())
}
