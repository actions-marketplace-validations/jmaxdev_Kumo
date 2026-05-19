use anyhow::Result;
use kumo_core::Store;
use resolver::Resolver;
use security::{Policy, SecurityEngine};
use std::path::PathBuf;
use serde_json;
use reqwest;
use semver;

pub async fn init_components() -> Result<(Store, SecurityEngine, Resolver)> {
    let store_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kumo")
        .join("store");

    let store = Store::new(store_path);
    store.init().await?;

    let mut config_json = serde_json::to_value(Policy::default())?;

    let global_config_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kumo")
        .join("kumo.config.json");
    if let Ok(content) = std::fs::read_to_string(&global_config_path) {
        if let Ok(global_val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = global_val.as_object() {
                for (k, v) in obj {
                    config_json[k] = v.clone();
                }
            }
        }
    }

    let local_config_path = std::env::current_dir()?.join("kumo.config.json");
    if let Ok(content) = std::fs::read_to_string(&local_config_path) {
        if let Ok(local_val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = local_val.as_object() {
                for (k, v) in obj {
                    config_json[k] = v.clone();
                }
            }
        }
    }

    let policy: Policy = serde_json::from_value(config_json)?;

    let security = SecurityEngine::new(policy);
    let resolver = Resolver::new();

    Ok((store, security, resolver))
}

pub fn get_deps_dir() -> String {
    let mut use_node_modules = false;
    let mut local_set = false;

    if let Ok(content) = std::fs::read_to_string("kumo.config.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(b) = v["useNodeModules"].as_bool() {
                use_node_modules = b;
                local_set = true;
            }
        }
    }

    if !local_set {
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".kumo").join("kumo.config.json");
            if let Ok(content) = std::fs::read_to_string(global_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    use_node_modules = v["useNodeModules"].as_bool().unwrap_or(false);
                }
            }
        }
    }

    if use_node_modules || std::path::Path::new("node_modules").exists() {
        "node_modules".to_string()
    } else {
        "dependencies".to_string()
    }
}

pub async fn check_for_new_version() -> Option<String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let home = dirs::home_dir()?;
    let check_file = home.join(".kumo").join("last_check.json");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let mut last_check = 0;
    let mut cached_latest = String::new();

    if check_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&check_file) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                last_check = v["timestamp"].as_u64().unwrap_or(0);
                cached_latest = v["version"].as_str().unwrap_or("").to_string();
            }
        }
    }


    if now - last_check > 86400 || cached_latest.is_empty() {
        let client = reqwest::Client::builder()
            .user_agent("kumo-pkg-manager")
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()?;

        let response = client
            .get("https://api.github.com/repos/jmaxdev/kumo/releases/latest")
            .send()
            .await
            .ok()?;

        if response.status().is_success() {
            if let Ok(release) = response.json::<serde_json::Value>().await {
                if let Some(tag) = release["tag_name"].as_str() {
                    cached_latest = tag.trim_start_matches('v').to_string();
                    let new_check_data = serde_json::json!({
                        "timestamp": now,
                        "version": cached_latest
                    });
                    let _ = std::fs::create_dir_all(check_file.parent().unwrap());
                    if let Ok(json) = serde_json::to_string(&new_check_data) {
                        let _ = std::fs::write(&check_file, json);
                    }
                }
            }
        }
    }

    if !cached_latest.is_empty() && cached_latest != current_version {
        let v_latest = semver::Version::parse(&cached_latest).ok()?;
        let v_current = semver::Version::parse(current_version).ok()?;
        if v_latest > v_current {
            return Some(cached_latest);
        }
    }

    None
}
