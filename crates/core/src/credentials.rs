use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RegistryCredential {
    pub token: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Credentials {
    pub registries: HashMap<String, RegistryCredential>,
}

fn get_credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".kumo").join("credentials.json"))
}

pub fn load_credentials() -> Result<Credentials> {
    let path = get_credentials_path()?;
    if !path.exists() {
        return Ok(Credentials::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let creds = serde_json::from_str(&content).unwrap_or_default();
    Ok(creds)
}

pub fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = get_credentials_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(creds)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub fn get_token(registry_url: &str) -> Option<String> {
    let creds = load_credentials().ok()?;
    let normalized = registry_url.trim_end_matches('/');
    creds.registries.get(normalized).map(|c| c.token.clone())
}

pub fn set_credential(registry_url: &str, username: String, token: String) -> Result<()> {
    let mut creds = load_credentials()?;
    let normalized = registry_url.trim_end_matches('/').to_string();
    creds
        .registries
        .insert(normalized, RegistryCredential { token, username });
    save_credentials(&creds)?;
    Ok(())
}
