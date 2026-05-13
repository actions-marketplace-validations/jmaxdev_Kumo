use anyhow::{Context, Result};
use blake3::Hasher;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn get_root(&self) -> &Path {
        &self.root
    }

    /// Adds a file to the content-addressable store.
    /// Returns the BLAKE3 hash of the file.
    pub async fn add_file(&self, content: &[u8]) -> Result<String> {
        let mut hasher = Hasher::new();
        hasher.update(content);
        let hash = hasher.finalize().to_hex().to_string();

        let path = self.get_path(&hash);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(path, content).await?;
        }

        Ok(hash)
    }

    /// Gets the path to a file in the store based on its hash.
    pub fn get_path(&self, hash: &str) -> PathBuf {
        self.root.join("objects").join(&hash[0..2]).join(&hash[2..])
    }

    /// Ensures the store directory exists.
    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .await
            .context("Failed to create store root")?;
        fs::create_dir_all(self.root.join("metadata"))
            .await
            .context("Failed to create store metadata dir")?;
        fs::create_dir_all(self.root.join("objects"))
            .await
            .context("Failed to create store objects dir")?;
        Ok(())
    }

    /// Saves a package index (file map) to the store.
    pub async fn save_index(&self, key: &str, file_map: &HashMap<String, String>) -> Result<()> {
        let path = self.get_index_path(key);
        let json = serde_json::to_string(file_map)?;
        fs::write(path, json)
            .await
            .context("Failed to save package index")
    }

    /// Loads a package index if it exists.
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

    fn get_index_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        let safe_key = key.replace('/', "__").replace('@', "@@");
        self.root
            .join("metadata")
            .join(format!("{}.json", safe_key))
    }
}
