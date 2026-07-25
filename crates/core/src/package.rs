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

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(64));
    let mut tasks = futures_util::stream::FuturesUnordered::new();

    for (rel_path, hash) in file_map {
        let store = store.clone();
        let target_dir = target_dir.to_path_buf();
        let rel_path = rel_path.clone();
        let hash = hash.clone();
        let sem = semaphore.clone();

        tasks.push(async move {
            let _permit = sem.acquire().await.unwrap();
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
                let mut attempts = 0;
                loop {
                    #[cfg(not(target_os = "windows"))]
                    {
                        if reflink_copy::reflink(&src_clone, &dest_clone).is_ok() {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if rel_path.contains("/bin/") || rel_path.starts_with("bin/") || rel_path.contains(".bin") || rel_path.ends_with(".sh") {
                                    let _ = std::fs::set_permissions(&dest_clone, std::fs::Permissions::from_mode(0o755));
                                }
                            }
                            return Ok(());
                        }
                    }
                    if std::fs::hard_link(&src_clone, &dest_clone).is_ok() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if rel_path.contains("/bin/") || rel_path.starts_with("bin/") || rel_path.contains(".bin") || rel_path.ends_with(".sh") {
                                let _ = std::fs::set_permissions(&dest_clone, std::fs::Permissions::from_mode(0o755));
                            }
                        }
                        return Ok(());
                    }
                    match std::fs::copy(&src_clone, &dest_clone) {
                        Ok(_) => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if rel_path.contains("/bin/") || rel_path.starts_with("bin/") || rel_path.contains(".bin") || rel_path.ends_with(".sh") {
                                    let _ = std::fs::set_permissions(&dest_clone, std::fs::Permissions::from_mode(0o755));
                                }
                            }
                            return Ok(());
                        },

                        Err(e) => {
                            attempts += 1;
                            if attempts >= 10 {
                                return Err(anyhow::anyhow!(
                                    "IO Error: {} (Source: {:?}, Dest: {:?})",
                                    e,
                                    src_clone,
                                    dest_clone
                                ));
                            }
                            let raw_err = e.raw_os_error();
                            if raw_err == Some(32) || raw_err == Some(5) {
                                std::thread::sleep(std::time::Duration::from_millis(15));
                                if dest_clone.exists() {
                                    let _ = std::fs::remove_file(&dest_clone);
                                }
                            } else {
                                return Err(anyhow::anyhow!(
                                    "IO Error: {} (Source: {:?}, Dest: {:?})",
                                    e,
                                    src_clone,
                                    dest_clone
                                ));
                            }
                        }
                    }
                }
            })
            .await?;

            result.map_err(|e| anyhow::anyhow!("Failed to link/copy package file: {}", e))
        });
    }

    use futures_util::StreamExt;
    while let Some(res) = tasks.next().await {
        res?;
    }

    Ok(())
}
