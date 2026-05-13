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

    let policy = Policy::default();
    let security = SecurityEngine::new(policy);
    let resolver = Resolver::new();

    Ok((store, security, resolver))
}
