use anyhow::{Context, Result};
use blake3::Hasher;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::shield::ShieldManager;

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
    shield: ShieldManager,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            shield: ShieldManager::new(),
        }
    }

    pub fn get_root(&self) -> &Path {
        &self.root
    }

    pub async fn add_file(&self, content: &[u8]) -> Result<String> {
        let mut hasher = Hasher::new();
        hasher.update(content);
        let hash = hasher.finalize().to_hex().to_string();

        let path = self.get_path(&hash);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let tmp_path = path.with_extension(format!("tmp-{}-{}", std::process::id(), now));

            if let Err(e) = fs::write(&tmp_path, content).await {
                let _ = fs::remove_file(&tmp_path).await;
                return Err(e.into());
            }

            if let Err(e) = fs::rename(&tmp_path, &path).await {
                let _ = fs::remove_file(&tmp_path).await;
                if !path.exists() {
                    return Err(e.into());
                }
            }

            if self.shield.is_active() {
                let _ = self.shield.shield_file(&path);
            }
        }

        Ok(hash)
    }

    pub fn get_path(&self, hash: &str) -> PathBuf {
        self.root.join(crate::config::OBJECTS_DIR_NAME).join(&hash[0..2]).join(&hash[2..])
    }

    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .await
            .context("Failed to create store root")?;
        fs::create_dir_all(self.root.join(crate::config::METADATA_DIR_NAME))
            .await
            .context("Failed to create store metadata dir")?;
        fs::create_dir_all(self.root.join(crate::config::OBJECTS_DIR_NAME))
            .await
            .context("Failed to create store objects dir")?;
        Ok(())
    }

    pub async fn save_index(&self, key: &str, file_map: &HashMap<String, String>) -> Result<()> {
        let path = self.get_index_path(key);
        let json = serde_json::to_string(file_map)?;
        if self.shield.is_active() {
            let _ = self.shield.unshield_file(&path);
        }
        let result = fs::write(&path, json)
            .await
            .context("Failed to save package index");
        if self.shield.is_active() {
            let _ = self.shield.shield_file(&path);
        }
        result
    }

    pub async fn load_index(&self, key: &str) -> Result<Option<HashMap<String, String>>> {
        let path = self.get_index_path(key);
        if path.exists() {
            let content = fs::read_to_string(path).await?;
            let map = serde_json::from_str(&content)?;
            Ok(Some(map))
        } else {
            Ok(None)
        }
    }

    pub async fn prune(&self) -> Result<u64> {
        let mut referenced_hashes = std::collections::HashSet::new();

        let mut metadata_entries = fs::read_dir(self.root.join(crate::config::METADATA_DIR_NAME)).await?;
        while let Some(entry) = metadata_entries.next_entry().await? {
            let content = fs::read_to_string(entry.path()).await?;
            let map: HashMap<String, String> = serde_json::from_str(&content)?;
            for hash in map.values() {
                referenced_hashes.insert(hash.clone());
            }
        }

        let mut deleted_count = 0;
        let objects_root = self.root.join(crate::config::OBJECTS_DIR_NAME);
        let mut dir_entries = fs::read_dir(&objects_root).await?;
        while let Some(dir_entry) = dir_entries.next_entry().await? {
            if dir_entry.file_type().await?.is_dir() {
                let mut obj_entries = fs::read_dir(dir_entry.path()).await?;
                while let Some(obj_entry) = obj_entries.next_entry().await? {
                    let hash_suffix = obj_entry.file_name().to_string_lossy().to_string();
                    let prefix = dir_entry.file_name().to_string_lossy().to_string();
                    let full_hash = format!("{}{}", prefix, hash_suffix);

                    if !referenced_hashes.contains(&full_hash) {
                        let _ = self.shield.unshield_file(&obj_entry.path());
                        fs::remove_file(obj_entry.path()).await?;
                        deleted_count += 1;
                    }
                }

                if fs::read_dir(dir_entry.path())
                    .await?
                    .next_entry()
                    .await?
                    .is_none()
                {
                    fs::remove_dir(dir_entry.path()).await?;
                }
            }
        }

        Ok(deleted_count)
    }

    fn get_index_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace('/', "__").replace('@', "@@");
        self.root
            .join(crate::config::METADATA_DIR_NAME)
            .join(format!("{}.json", safe_key))
    }
}
