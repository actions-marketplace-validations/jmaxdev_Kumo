use anyhow::Result;
use kumo_core::Store;

#[derive(clap::Args)]
pub struct DoctorCommand;

#[async_trait::async_trait(?Send)]
impl super::Command for DoctorCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(&ctx.store).await
    }
}

pub async fn execute(store: &Store) -> Result<()> {
    println!("Kumo Doctor: Running health checks...\n");
    let mut issues = 0;

    let root = store.get_root();
    if root.exists() {
        println!("  \x1b[32m✓\x1b[0m Global store exists at {:?}", root);
    } else {
        println!("  \x1b[33m⚠\x1b[0m Global store not found. Running init...");
        store.init().await?;
        issues += 1;
    }

    let node_version = std::process::Command::new("node").arg("--version").output();
    match node_version {
        Ok(output) => println!(
            "  \x1b[32m✓\x1b[0m Node.js found: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(_) => {
            println!("  \x1b[31m✗\x1b[0m Node.js not found in PATH");
            issues += 1;
        }
    }

    let objects_dir = root.join("objects");
    if objects_dir.exists() {
        println!("\n  Verifying BLAKE3 integrity of cached objects...");
        let mut total = 0u64;
        let mut corrupted = 0u64;

        let mut prefix_entries = tokio::fs::read_dir(&objects_dir).await?;
        while let Some(prefix_entry) = prefix_entries.next_entry().await? {
            if !prefix_entry.file_type().await?.is_dir() {
                continue;
            }
            let prefix = prefix_entry.file_name().to_string_lossy().to_string();

            let mut obj_entries = tokio::fs::read_dir(prefix_entry.path()).await?;
            while let Some(obj_entry) = obj_entries.next_entry().await? {
                total += 1;
                let suffix = obj_entry.file_name().to_string_lossy().to_string();
                let expected_hash = format!("{}{}", prefix, suffix);

                let content = tokio::fs::read(obj_entry.path()).await?;
                let mut hasher = blake3::Hasher::new();
                hasher.update(&content);
                let actual_hash = hasher.finalize().to_hex().to_string();

                if actual_hash != expected_hash {
                    corrupted += 1;
                    eprintln!(
                        "  \x1b[31m✗\x1b[0m Corrupted: {} (expected {}, got {})",
                        obj_entry.path().display(),
                        &expected_hash[..8],
                        &actual_hash[..8]
                    );
                }
            }
        }

        if corrupted == 0 {
            println!(
                "  \x1b[32m✓\x1b[0m All {} objects passed BLAKE3 integrity check.",
                total
            );
        } else {
            println!(
                "  \x1b[31m✗\x1b[0m {}/{} objects are corrupted! Run 'kumo prune store' and reinstall.",
                corrupted, total
            );
            issues += corrupted as usize;
        }
    } else {
        println!("  \x1b[33m⚠\x1b[0m No objects directory found. Store is empty.");
    }

    let metadata_dir = root.join("metadata");
    if metadata_dir.exists() {
        let mut meta_count = 0u64;
        let mut broken_meta = 0u64;
        let mut meta_entries = tokio::fs::read_dir(&metadata_dir).await?;
        while let Some(entry) = meta_entries.next_entry().await? {
            meta_count += 1;
            let content = tokio::fs::read_to_string(entry.path()).await?;
            if serde_json::from_str::<serde_json::Value>(&content).is_err() {
                broken_meta += 1;
                eprintln!(
                    "  \x1b[31m✗\x1b[0m Corrupted metadata: {}",
                    entry.path().display()
                );
            }
        }
        if broken_meta == 0 {
            println!(
                "  \x1b[32m✓\x1b[0m All {} metadata entries are valid JSON.",
                meta_count
            );
        } else {
            println!(
                "  \x1b[31m✗\x1b[0m {}/{} metadata files are corrupted.",
                broken_meta, meta_count
            );
            issues += broken_meta as usize;
        }
    }

    println!();
    if issues == 0 {
        println!("\x1b[32mHealth check complete. No issues found.\x1b[0m");
    } else {
        println!(
            "\x1b[33mHealth check complete. {} issue(s) detected.\x1b[0m",
            issues
        );
    }
    Ok(())
}
