use anyhow::Result;
use kumo_core::Store;

pub async fn execute(store: &Store) -> Result<()> {
    let root = store.get_root();
    let objects_dir = root.join("objects");
    let mut total_size = 0;
    let mut file_count = 0;

    if objects_dir.exists() {
        let mut entries = tokio::fs::read_dir(objects_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let mut files = tokio::fs::read_dir(entry.path()).await?;
                while let Some(file) = files.next_entry().await? {
                    total_size += file.metadata().await?.len();
                    file_count += 1;
                }
            }
        }
    }

    println!("Kumo Global Store Stats:");
    println!("Location: {:?}", root);
    println!("Total objects: {}", file_count);
    println!("Total size: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
    Ok(())
}
