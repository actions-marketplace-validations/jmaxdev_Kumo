use anyhow::Result;
use clap::Subcommand;
use kumo_core::shield::ShieldManager;

#[derive(Subcommand, Clone)]
pub enum ShieldAction {
    #[command(about = "Enable Kumo Shield to protect dependencies from unauthorized modification")]
    On,
    #[command(about = "Disable Kumo Shield")]
    Off,
    #[command(about = "Check current Kumo Shield status")]
    Status,
}

#[derive(clap::Args)]
pub struct ShieldCommand {
    #[command(subcommand)]
    pub action: ShieldAction,
}

#[async_trait::async_trait(?Send)]
impl super::Command for ShieldCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.action.clone()).await
    }
}

pub async fn execute(action: ShieldAction) -> Result<()> {
    let shield = ShieldManager::new();
    
    match action {
        ShieldAction::On => {
            shield.set_active(true)?;
            
            println!("🛡️  Kumo Shield activated!");
            
            // Retroactively shield the global store and cache
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let objects_dir = home.join(".kumo").join("store").join("objects");
            let metadata_dir = home.join(".kumo").join("store").join("metadata");
            let cache_dir = home.join(".kumo").join("cache");

            println!("Applying protection to existing cached packages and metadata...");
            if objects_dir.exists() {
                let _ = shield.shield_dir_recursive(&objects_dir);
            }
            if metadata_dir.exists() {
                let _ = shield.shield_dir_recursive(&metadata_dir);
            }
            if cache_dir.exists() {
                let _ = shield.shield_dir_recursive(&cache_dir);
            }

            // Retroactively shield local project files
            if let Ok(cwd) = std::env::current_dir() {
                for file in ["kumo.lock", "kumo.json", "kumo.config.json"] {
                    let path = cwd.join(file);
                    if path.exists() {
                        let _ = shield.shield_file(&path);
                    }
                }
            }

            println!("Existing and new packages in the cache are now marked as Read-Only.");
            println!("To edit kumo.config.json or kumo.lock, use 'kumo unlock <file>'.");
        }
        ShieldAction::Off => {
            shield.set_active(false)?;
            
            println!("🔓 Kumo Shield disabled.");
            println!("Packages and configurations can now be modified freely.");
        }
        ShieldAction::Status => {
            if shield.is_active() {
                println!("🛡️  Kumo Shield is currently ON");
            } else {
                println!("🔓 Kumo Shield is currently OFF");
            }
        }
    }
    
    Ok(())
}
