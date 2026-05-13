use std::path::PathBuf;
use anyhow::Result;
use core::Store;
use security::{SecurityEngine, Policy};
use resolver::Resolver;

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
