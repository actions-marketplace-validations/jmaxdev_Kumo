use anyhow::Result;
use kumo_core::Store;
use resolver::Resolver;
use security::SecurityEngine;
use std::collections::HashMap;

pub async fn execute(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    name: String,
    dev: bool,
    global: bool,
    log: bool,
    config_path: Option<std::path::PathBuf>,
) -> Result<()> {
    if global {
        crate::commands::install::install_global(store, resolver, security, name).await?;
    } else {
        let config_path = config_path.ok_or_else(|| {
            anyhow::anyhow!("Neither kumo.json nor package.json found in current directory")
        })?;
        println!("Adding {} to configuration...", name);
        let mut config_content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
        let section = if dev {
            "devDependencies"
        } else {
            "dependencies"
        };
        if let Some(obj) = config_content.as_object_mut() {
            obj.entry(section.to_string())
                .or_insert(serde_json::json!({}))
                .as_object_mut()
                .unwrap()
                .insert(name.clone(), serde_json::json!("latest"));
        }

        let json = serde_json::to_string_pretty(&config_content)?;
        std::fs::write(&config_path, json)?;
        println!(
            "Updated {} with {}",
            config_path.file_name().unwrap().to_string_lossy(),
            name
        );

        let mut deps = HashMap::new();
        deps.insert(name.clone(), "latest".to_string());
        crate::commands::install::resolve_and_install(
            store,
            resolver,
            security,
            deps,
            log,
            config_path,
        )
        .await?;
    }
    Ok(())
}
