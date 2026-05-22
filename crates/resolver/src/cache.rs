use std::path::PathBuf;

pub fn get_metadata_cache_path(name: &str) -> PathBuf {
    let cache_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kumo")
        .join("cache")
        .join("metadata");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("{}.json", name.replace('/', "__")))
}
