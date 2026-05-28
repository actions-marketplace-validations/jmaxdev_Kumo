use anyhow::Result;
use clap::Subcommand;
use kumo_core::shield::ShieldManager;

#[derive(Subcommand)]
pub enum ShieldAction {
    #[command(about = "Enable Kumo Shield to protect dependencies from unauthorized modification")]
    On,
    #[command(about = "Disable Kumo Shield")]
    Off,
    #[command(about = "Check current Kumo Shield status")]
    Status,
}

pub async fn execute(action: ShieldAction) -> Result<()> {
    let shield = ShieldManager::new();
    
    match action {
        ShieldAction::On => {
            shield.set_active(true)?;
            
            println!("🛡️  Kumo Shield activated!");
            println!("New packages added to the cache will be marked as Read-Only.");
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
