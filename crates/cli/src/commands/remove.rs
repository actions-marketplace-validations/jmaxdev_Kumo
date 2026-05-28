use anyhow::Result;
use kumo_core::shield::ShieldManager;
use kumo_core::Store;
use resolver::Resolver;
use security::SecurityEngine;
use std::collections::HashMap;
use crate::common;

pub async fn execute(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    name: String,
    config_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let config_path = config_path.ok_or_else(|| {
        anyhow::anyhow!("Neither kumo.json nor package.json found in current directory")
    })?;
    println!("Removing {}...", name);
    let mut config_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

    let mut removed = false;
    if let Some(deps) = config_content
        .get_mut("dependencies")
        .and_then(|v| v.as_object_mut())
    {
        if deps.remove(&name).is_some() {
            removed = true;
        }
    }
    if let Some(deps) = config_content
        .get_mut("devDependencies")
        .and_then(|v| v.as_object_mut())
    {
        if deps.remove(&name).is_some() {
            removed = true;
        }
    }

    if !removed {
        println!("Package {} not found in dependencies.", name);
    } else {
        let json = serde_json::to_string_pretty(&config_content)?;
        let shield = ShieldManager::new();
        if shield.is_active() {
            let _ = shield.unshield_file(&config_path);
        }
        std::fs::write(&config_path, json)?;
        println!(
            "Removed {} from {}",
            name,
            config_path.file_name().unwrap().to_string_lossy()
        );

        let deps_dir = common::get_deps_dir();
        let pkg_dir = std::env::current_dir()?.join(&deps_dir).join(&name);
        if pkg_dir.exists() {
            let _ = std::fs::remove_dir_all(&pkg_dir);
        }

        println!("Updating lockfile and cleaning up...");
        let mut deps = HashMap::new();
        if let Some(d) = config_content
            .get("dependencies")
            .and_then(|v| v.as_object())
        {
            for (k, v) in d {
                deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
            }
        }
        if let Some(d) = config_content
            .get("devDependencies")
            .and_then(|v| v.as_object())
        {
            for (k, v) in d {
                deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
            }
        }

        crate::commands::install::resolve_and_install(
            store,
            resolver,
            security,
            deps,
            false,
            config_path,
        )
        .await?;
    }
    Ok(())
}
