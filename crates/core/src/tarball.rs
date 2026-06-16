use crate::Store;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use std::collections::HashMap;
use std::io::Read;
use tar::Archive;
use tokio::sync::mpsc;
use tokio_util::io::SyncIoBridge;

pub fn verify_shasum(tarball_data: &[u8], expected_shasum: &str) -> Result<()> {
    use sha1::{Sha1, Digest};
    let mut hasher = Sha1::new();
    hasher.update(tarball_data);
    let result = hasher.finalize();
    let actual_shasum = result.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    if actual_shasum != expected_shasum {
        anyhow::bail!(
            "Tarball integrity check failed!\n  Expected SHA-1: {}\n  Actual SHA-1:   {}\n  This could indicate a corrupted download or a supply chain attack.",
            expected_shasum,
            actual_shasum
        );
    }
    Ok(())
}

pub async fn extract_streaming<S>(store: &Store, stream: S) -> Result<HashMap<String, String>>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static + Unpin,
{
    let (tx, mut rx) = mpsc::channel::<(String, Vec<u8>)>(32);

    let stream = stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let async_reader = tokio_util::io::StreamReader::new(stream);
    let sync_reader = SyncIoBridge::new(async_reader);

    let extractor_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut archive = Archive::new(GzDecoder::new(sync_reader));

        for entry in archive
            .entries()
            .context("Failed to read tarball entries")?
        {
            let mut entry = entry.context("Failed to read entry")?;

            if !entry.header().entry_type().is_file() {
                continue;
            }

            let path = entry.path()?.to_string_lossy().to_string();
            let clean_path = if path.starts_with("package/") {
                &path[8..]
            } else {
                &path
            };

            let mut buffer = Vec::new();
            entry
                .read_to_end(&mut buffer)
                .context("Failed to read file content")?;

            if tx.blocking_send((clean_path.to_string(), buffer)).is_err() {
                break;
            }
        }
        Ok(())
    });

    let mut file_map = HashMap::new();
    let mut storage_tasks = futures_util::stream::FuturesUnordered::new();

    let mut rx_closed = false;
    loop {
        tokio::select! {
            res = rx.recv(), if !rx_closed => {
                match res {
                    Some((path, content)) => {
                        let store = store.clone();
                        storage_tasks.push(async move {
                            let hash = store.add_file(&content).await?;
                            Ok::<(String, String), anyhow::Error>((path, hash))
                        });
                    }
                    None => rx_closed = true,
                }
            }
            res = storage_tasks.next(), if !storage_tasks.is_empty() => {
                if let Some(result) = res {
                    let (path, hash) = result?;
                    file_map.insert(path, hash);
                }
            }
            else => {
                if rx_closed && storage_tasks.is_empty() {
                    break;
                }
            }
        }
    }

    extractor_handle
        .await
        .context("Extractor task panicked")??;

    Ok(file_map)
}

pub async fn extract_and_store(
    store: &Store,
    tarball_data: &[u8],
) -> Result<HashMap<String, String>> {
    let mut archive = Archive::new(GzDecoder::new(tarball_data));
    let mut file_map = HashMap::new();

    for entry in archive
        .entries()
        .context("Failed to read tarball entries")?
    {
        let mut entry = entry.context("Failed to read entry")?;

        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path()?.to_string_lossy().to_string();

        let clean_path = if path.starts_with("package/") {
            &path[8..]
        } else {
            &path
        };

        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .context("Failed to read file content")?;

        let hash = store.add_file(&buffer).await?;
        file_map.insert(clean_path.to_string(), hash);
    }

    Ok(file_map)
}

pub fn pack_directory(dir: &std::path::Path) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        
        let walker = walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                name != "node_modules" 
                    && name != ".git" 
                    && name != ".kumo" 
                    && name != "target"
                    && name != ".DS_Store"
                    && name != "kumo.lock"
                    && name != "package-lock.json"
                    && name != "pnpm-lock.yaml"
                    && name != "yarn.lock"
            });

        for entry in walker {
            let entry = entry.context("Failed to read directory entry during packing")?;
            let path = entry.path();
            if path.is_file() {
                let rel_path = path.strip_prefix(dir)?;
                
                let mut components = vec!["package".to_string()];
                for comp in rel_path.components() {
                    components.push(comp.as_os_str().to_string_lossy().to_string());
                }
                let tar_name = components.join("/");
                
                builder.append_path_with_name(path, &tar_name)
                    .with_context(|| format!("Failed to append file {} to archive", path.display()))?;
            }
        }
        builder.finish()?;
    }
    
    let tarball_bytes = encoder.finish()?;
    Ok(tarball_bytes)
}

pub fn calculate_shasum(data: &[u8]) -> String {
    use sha1::{Sha1, Digest};
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn calculate_integrity(data: &[u8]) -> String {
    use sha2::{Sha512, Digest};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut hasher = Sha512::new();
    hasher.update(data);
    let hash = hasher.finalize();
    format!("sha512-{}", STANDARD.encode(&hash))
}

pub fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_shasum() {
        let data = b"hello world";
        let expected = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
        
        assert!(verify_shasum(data, expected).is_ok());
        assert!(verify_shasum(data, "wrong").is_err());
    }
}
