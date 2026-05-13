use anyhow::Result;
use kumo_core::Store;
use resolver::Resolver;
use security::{Policy, SecurityEngine};
use std::path::PathBuf;

pub async fn init_components() -> Result<(Store, SecurityEngine, Resolver)> {
    let store_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kumo")
        .join("store");

    let store = Store::new(store_path);
    store.init().await?;

    let config_path = std::env::current_dir()?.join("kumo.config.json");
    let policy = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;
        serde_json::from_str(&content)?
    } else {
        Policy::default()
    };

    let security = SecurityEngine::new(policy);
    let resolver = Resolver::new();

    Ok((store, security, resolver))
}

pub fn get_deps_dir() -> String {
    if std::path::Path::new("node_modules").exists() {
        "node_modules".to_string()
    } else {
        "dependencies".to_string()
    }
}
