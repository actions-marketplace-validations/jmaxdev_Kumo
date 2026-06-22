use anyhow::Result;
use clap::Subcommand;
use kumo_core::Store;
use kumo_core::shield::ShieldManager;
use std::path::PathBuf;

#[derive(Subcommand, Clone)]
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
        #[arg(help = "Path to start searching or pruning from. Defaults to current directory")]
        path: Option<String>,
    },
    #[command(about = "Clean both the global store and the registry cache")]
    All,
}

#[derive(clap::Args)]
pub struct PruneCommand {
    #[command(subcommand)]
    pub subcommand: PruneSubcommand,
}

#[async_trait::async_trait(?Send)]
impl super::Command for PruneCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(&ctx.store, self.subcommand.clone()).await
    }
}

pub async fn execute(store: &Store, subcommand: PruneSubcommand) -> Result<()> {
    match subcommand {
        PruneSubcommand::Store => {
            prune_store(store).await?;
        }
        PruneSubcommand::Cache => {
            prune_cache().await?;
        }
        PruneSubcommand::Deps { full, remove_all, path } => {

            prune_deps(full, remove_all, path).await?;
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

async fn prune_deps(full: bool, remove_all: bool, path: Option<String>) -> Result<()> {
    use indicatif::{ProgressBar, ProgressStyle};
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    let deps_dir = crate::common::get_deps_dir();
    let current_dir = if let Some(p) = path {
        std::path::PathBuf::from(p)
    } else {
        std::env::current_dir()?
    };

    if remove_all {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
                .template("{spinner:.green} {msg}")?,
        );
        spinner.set_message("Searching for dependency directories to remove...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let mut paths_to_delete = Vec::new();

        fn find_targets(dir: &std::path::Path, deps_name: &str, full: bool, targets: &mut Vec<std::path::PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy();
                            if name_str == "node_modules" || name_str == "dependencies" || name_str == deps_name {
                                targets.push(path);
                            } else if name_str != ".git" && name_str != ".kumo" && name_str != "target" {
                                find_targets(&path, deps_name, full, targets);
                            }
                        }
                    } else if path.is_file() && full {
                        if let Some(name) = path.file_name() {
                            if name == "kumo.lock" {
                                targets.push(path);
                            }
                        }
                    }
                }
            }
        }

        fn dir_size(path: &std::path::Path) -> u64 {
            let mut size = 0;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        size += dir_size(&p);
                    } else if let Ok(meta) = std::fs::metadata(&p) {
                        size += meta.len();
                    }
                }
            }
            size
        }

        find_targets(&current_dir, &deps_dir, full, &mut paths_to_delete);
        
        spinner.set_message("Calculating disk space usage...");
        
        let path_sizes: Vec<(std::path::PathBuf, u64)> = paths_to_delete
            .par_iter()
            .map(|path| {
                if path.is_file() {
                    let s = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    (path.clone(), s)
                } else {
                    (path.clone(), dir_size(path))
                }
            })
            .collect();

        spinner.finish_and_clear();

        if path_sizes.is_empty() {
            println!("No dependency directories found.");
            return Ok(());
        }

        let format_size = |size: u64| -> String {
            if size >= 1_048_576_000 {
                format!("{:.2} GB", size as f64 / 1_048_576_000.0)
            } else if size >= 1_048_576 {
                format!("{:.1} MB", size as f64 / 1_048_576.0)
            } else if size >= 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{} B", size)
            }
        };

        let mut total_size = 0;
        for (path, size) in &path_sizes {
            total_size += size;
            println!("📄 Found {} ({})", path.display(), format_size(*size));
        }

        let pb = ProgressBar::new(paths_to_delete.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
                .progress_chars("#>-"),
        );
        pb.set_message("Deleting directories...");

        let count = AtomicU64::new(0);

        paths_to_delete.par_iter().for_each(|path| {
            if path.is_dir() {
                if std::fs::remove_dir_all(path).is_ok() {
                    count.fetch_add(1, Ordering::SeqCst);
                }
            } else {
                let shield = ShieldManager::new();
                let _ = shield.unshield_file(path);
                if std::fs::remove_file(path).is_ok() {
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }
            pb.inc(1);
        });

        pb.finish_with_message(format!("✅ Removed {} items. Freed {} of disk space.", count.load(Ordering::SeqCst), format_size(total_size)));
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
                println!("Deleted local kumo.lock.");
            }
        }
    }

    Ok(())
}
