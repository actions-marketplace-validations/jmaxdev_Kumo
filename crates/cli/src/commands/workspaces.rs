use anyhow::Result;

#[derive(clap::Args)]
pub struct WorkspacesCommand;

#[async_trait::async_trait(?Send)]
impl super::Command for WorkspacesCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute().await
    }
}

pub async fn execute() -> Result<()> {
    println!("Kumo Workspaces: Detecting local packages...");
    let mut found = 0;
    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let pkg_json = entry.path().join("package.json");
                let kumo_json = entry.path().join("kumo.json");
                if pkg_json.exists() || kumo_json.exists() {
                    let path = if pkg_json.exists() {
                        pkg_json
                    } else {
                        kumo_json
                    };
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let v: serde_json::Value =
                            serde_json::from_str(&content).unwrap_or_default();
                        let name = v["name"].as_str().unwrap_or("unknown");
                        let version = v["version"].as_str().unwrap_or("0.0.0");
                        println!(" - {} (v{}) at {:?}", name, version, entry.path());
                        found += 1;
                    }
                }
            }
        }
    }
    if found == 0 {
        println!("No local workspaces found. Kumo supports monorepos with package.json/kumo.json in subdirectories.");
    } else {
        println!("Found {} local packages.", found);
    }
    Ok(())
}
