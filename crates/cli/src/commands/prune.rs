use anyhow::Result;
use clap::Subcommand;
use kumo_core::Store;
use kumo_core::shield::ShieldManager;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PruneSubcommand {
    #[command(about = "Clean the global content-addressable store (~/.kumo/store)")]
    Store,
    #[command(about = "Clean the registry metadata and scripts cache (~/.kumo/cache/metadata & ~/.kumo/cache/scripts)")]
    Cache,
    #[command(about = "Delete the local dependencies directory and optionally the lockfile")]
    Deps {
        #[arg(long, help = "Also delete kumo.lock")]
        full: bool,
        #[arg(long, help = "Recursively find and remove all dependency directories in subdirectories")]
        remove_all: bool,
    },
    #[command(about = "Clean both the global store and the registry cache")]
    All,
}

pub async fn execute(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Store => {
            prune_store(store).await?;
        }
        PruneSubcommand::Cache => {
            prune_cache().await?;
        }
        PruneSubcommand::Deps { full, remove_all } => {
            prune_deps(full, remove_all).await?;
        }
        PruneSubcommand::All => {
            prune_store(store).await?;
            prune_cache().await?;
        }
    }
    Ok(())
}

async fn prune_store(store: &Store) -> Result<()> {
    println!("Pruning global store...");
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
    Ok(())
}

async fn prune_cache() -> Result<()> {
    let cache_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kumo")
        .join("cache");

    if !cache_dir.exists() {
        println!("Registry cache is already empty.");
        return Ok(());
    }

    let mut count: u64 = 0;
    let mut size: u64 = 0;

    // Walk the cache directory and count files before deleting
    fn count_files(dir: &std::path::Path, count: &mut u64, size: &mut u64) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    count_files(&path, count, size);
                } else if path.is_file() {
                    *count += 1;
                    if let Ok(meta) = std::fs::metadata(&path) {
                        *size += meta.len();
                    }
                }
            }
        }
    }

    let metadata_dir = cache_dir.join("metadata");
    let scripts_dir = cache_dir.join("scripts");

    count_files(&metadata_dir, &mut count, &mut size);
    count_files(&scripts_dir, &mut count, &mut size);

    if count == 0 {
        println!("Registry cache is already empty.");
        return Ok(());
    }

    let shield = ShieldManager::new();
    if metadata_dir.exists() {
        let _ = shield.unshield_dir_recursive(&metadata_dir);
        std::fs::remove_dir_all(&metadata_dir)?;
        std::fs::create_dir_all(&metadata_dir)?;
    }
    if scripts_dir.exists() {
        let _ = shield.unshield_dir_recursive(&scripts_dir);
        std::fs::remove_dir_all(&scripts_dir)?;
        std::fs::create_dir_all(&scripts_dir)?;
    }

    let size_display = if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else if size >= 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{} B", size)
    };

    println!("Cleared {} cached entries ({}).", count, size_display);
    Ok(())
}

async fn prune_deps(full: bool, remove_all: bool) -> Result<()> {
    let deps_dir = crate::common::get_deps_dir();
    let current_dir = std::env::current_dir()?;

    if remove_all {
        println!("Searching for dependency directories to remove recursively...");
        
        fn find_and_remove(dir: &std::path::Path, deps_name: &str, full: bool, count: &mut u64) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy();
                            if name_str == "node_modules" || name_str == "dependencies" || name_str == deps_name {
                                println!("Deleting {}...", path.display());
                                if std::fs::remove_dir_all(&path).is_ok() {
                                    *count += 1;
                                }
                            } else if name_str != ".git" && name_str != ".kumo" && name_str != "target" {
                                find_and_remove(&path, deps_name, full, count);
                            }
                        }
                    } else if path.is_file() && full {
                        if let Some(name) = path.file_name() {
                            if name == "kumo.lock" {
                                println!("Deleting {}...", path.display());
                                let shield = ShieldManager::new();
                                let _ = shield.unshield_file(&path);
                                if std::fs::remove_file(&path).is_ok() {
                                    *count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut count = 0;
        find_and_remove(&current_dir, &deps_dir, full, &mut count);
        
        println!("Removed {} items in total.", count);
    } else {
        println!("Pruning {} directory...", deps_dir);
        let deps_path = current_dir.join(&deps_dir);
        if deps_path.exists() {
            std::fs::remove_dir_all(&deps_path)?;
            println!("Deleted local {} directory.", deps_dir);
        }
        if full {
            let lock_path = current_dir.join("kumo.lock");
            if lock_path.exists() {
                let shield = ShieldManager::new();
                let _ = shield.unshield_file(&lock_path);
                std::fs::remove_file(lock_path)?;
                println!("Deleted kumo.lock");
            }
        }
    }
    Ok(())
}
