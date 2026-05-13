use crate::Store;
use anyhow::{Result, Context};
use std::path::Path;
use std::io::Read;
use tar::Archive;
use flate2::read::GzDecoder;
use std::collections::HashMap;

/// Extracts a tarball and adds all its files to the CAS store.
/// Returns a map of relative file paths within the package to their content hashes.
pub async fn extract_and_store(store: &Store, tarball_data: &[u8]) -> Result<HashMap<String, String>> {
    let mut archive = Archive::new(GzDecoder::new(tarball_data));
    let mut file_map = HashMap::new();

    for entry in archive.entries().context("Failed to read tarball entries")? {
        let mut entry = entry.context("Failed to read entry")?;
        
        // Skip directories, we only care about files
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path()?.to_string_lossy().to_string();
        
        // Clean the path (remove leading 'package/' which is common in npm tarballs)
        let clean_path = if path.starts_with("package/") {
            &path[8..]
        } else {
            &path
        };

        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).context("Failed to read file content")?;
        
        let hash = store.add_file(&buffer).await?;
        file_map.insert(clean_path.to_string(), hash);
    }

    Ok(file_map)
}
