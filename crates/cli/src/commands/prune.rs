use anyhow::Result;
use clap::Subcommand;
use kumo_core::Store;
use kumo_core::shield::ShieldManager;

#[derive(Subcommand)]
pub enum PruneSubcommand {
    Cache {
        #[arg(long)]
        full: bool,
    },
    Deps {
        #[arg(long)]
        full: bool,
    },
    Kx {
        #[arg(long)]
        full: bool,
    },
}

pub async fn execute(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Cache { full } => {
            if full {
                println!("Performing FULL prune of global store...");
                let root = store.get_root();
                let metadata_dir = root.join("metadata");
                let objects_dir = root.join("objects");
                let shield = ShieldManager::new();

                if metadata_dir.exists() {
                    let _ = shield.unshield_dir_recursive(&metadata_dir);
                    let _ = std::fs::remove_dir_all(&metadata_dir);
                    let _ = std::fs::create_dir_all(&metadata_dir);
                }
                if objects_dir.exists() {
                    let _ = shield.unshield_dir_recursive(&objects_dir);
                    let _ = std::fs::remove_dir_all(&objects_dir);
                    let _ = std::fs::create_dir_all(&objects_dir);
                }
                println!("Global store cleared.");
            } else {
                println!("Pruning unreferenced global store objects...");
                let deleted = store.prune().await?;
                println!("Cleaned up {} unreferenced objects.", deleted);
            }
        }
        PruneSubcommand::Deps { full } => {
            let deps_dir = crate::common::get_deps_dir();
            println!("Pruning {} directory...", deps_dir);
            if std::path::Path::new(&deps_dir).exists() {
                std::fs::remove_dir_all(&deps_dir)?;
                println!("Deleted local {} directory.", deps_dir);
            }
            if full {
                let lock_path = std::env::current_dir()?.join("kumo.lock");
                if lock_path.exists() {
                    let shield = ShieldManager::new();
                    let _ = shield.unshield_file(&lock_path);
                    std::fs::remove_file(lock_path)?;
                    println!("Deleted kumo.lock");
                }
            }
        }
        PruneSubcommand::Kx { full } => {
            let kx_root = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".kumo")
                .join("kx");
            if !kx_root.exists() {
                println!("KX cache is already empty.");
                return Ok(());
            }

            if full {
                println!("Performing FULL prune of KX cache...");
                std::fs::remove_dir_all(&kx_root)?;
                std::fs::create_dir_all(&kx_root)?;
                println!("KX cache cleared.");
            } else {
                println!("Pruning old KX packages (older than 7 days)...");
                let mut count = 0;
                let entries = std::fs::read_dir(&kx_root)?;
                let now = std::time::SystemTime::now();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            let accessed = metadata.accessed().unwrap_or_else(|_| {
                                metadata.modified().unwrap_or(now)
                            });
                            if now.duration_since(accessed).map(|d| d.as_secs() > 7 * 24 * 3600).unwrap_or(false) {
                                let _ = std::fs::remove_dir_all(&path);
                                count += 1;
                            }
                        }
                    }
                }
                println!("Removed {} old KX packages.", count);
            }
        }
    }
    Ok(())
}
