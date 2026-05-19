use anyhow::Result;
use kumo_core::Store;

pub async fn execute(store: &Store) -> Result<()> {
    println!("Kumo Doctor: Checking system health...");
    let root = store.get_root();
    if root.exists() {
        println!("[OK] Global store exists at {:?}", root);
    } else {
        println!("[WARN] Global store not found. Running init...");
        store.init().await?;
    }

    let node_version = std::process::Command::new("node").arg("--version").output();
    match node_version {
        Ok(output) => println!(
            "[OK] Node.js found: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(_) => println!("[ERROR] Node.js not found in PATH"),
    }

    println!("Health check complete.");
    Ok(())
}
