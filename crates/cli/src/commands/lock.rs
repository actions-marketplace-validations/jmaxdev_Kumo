use anyhow::Result;
use kumo_core::shield::ShieldManager;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct LockCommand {
    pub file: Option<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for LockCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.file.clone()).await
    }
}

pub async fn execute(file: Option<String>) -> Result<()> {
    let shield = ShieldManager::new();
    
    if !shield.is_active() {
        println!("🔓 Shield is not active.");
        return Ok(());
    }

    let files_to_lock = match file {
        Some(f) => vec![f],
        None => vec!["kumo.config.json".to_string(), "kumo.lock".to_string()],
    };

    let cwd = std::env::current_dir()?;

    for f in files_to_lock {
        let path = cwd.join(&f);
        if path.exists() {
            shield.shield_file(&path)?;
            println!("🛡️  Locked {}", f);
        } else if f == "kumo.config.json" {
            let global_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".kumo").join("kumo.config.json");
            if global_path.exists() {
                shield.shield_file(&global_path)?;
                println!("🛡️  Locked global kumo.config.json");
            }
        }
    }

    Ok(())
}
