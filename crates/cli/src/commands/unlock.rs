use anyhow::{Context, Result};
use dialoguer::Confirm;
use kumo_core::shield::ShieldManager;
use std::path::{Path, PathBuf};

pub async fn execute(file: String) -> Result<()> {
    let shield = ShieldManager::new();
    
    if !shield.is_active() {
        println!("🔓 Shield is not active. Files can be modified normally.");
        return Ok(());
    }

    // Whitelist check
    let allowed_files = ["kumo.config.json", "kumo.lock"];
    let filename = Path::new(&file).file_name().unwrap_or_default().to_string_lossy();
    
    if !allowed_files.contains(&filename.as_ref()) {
        anyhow::bail!("Security violation: Only kumo.config.json and kumo.lock can be unlocked.");
    }

    let file_path = std::env::current_dir()?.join(&file);
    if !file_path.exists() {
        // Also check if it might be global config
        if file == "kumo.config.json" {
            let global_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".kumo").join("kumo.config.json");
            if global_path.exists() {
                return unlock_file(&global_path, &shield).await;
            }
        }
        anyhow::bail!("File {} not found.", file);
    }

    unlock_file(&file_path, &shield).await
}

async fn unlock_file(path: &Path, shield: &ShieldManager) -> Result<()> {
    // VULN-6: TTY Check
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Security violation: kumo unlock requires an interactive terminal (TTY) to verify human presence.");
    }

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    println!("⚠️  You are about to unlock {} which is currently protected by Kumo Shield.", file_name);
    
    let confirm = Confirm::new()
        .with_prompt(format!("Are you sure you want to unlock {}?", file_name))
        .default(false)
        .interact()
        .context("Failed to read user confirmation")?;

    if confirm {
        shield.unshield_file(path)?;
        println!("🔓 {} is now unlocked and writable.", file_name);
        println!("Run 'kumo lock' or any kumo command when you are done to re-shield it.");
    } else {
        println!("Operation cancelled.");
    }

    Ok(())
}
