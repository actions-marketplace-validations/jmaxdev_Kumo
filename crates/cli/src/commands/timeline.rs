use anyhow::Result;
use resolver::Lockfile;

#[derive(clap::Args)]
pub struct TimelineCommand;

#[async_trait::async_trait(?Send)]
impl super::Command for TimelineCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute().await
    }
}

pub async fn execute() -> Result<()> {
    let lock_path = std::env::current_dir()?.join("kumo.lock");
    if let Ok(metadata) = std::fs::metadata(&lock_path) {
        let created = metadata.created().unwrap_or(metadata.modified().unwrap());
        let modified = metadata.modified().unwrap();
        println!("Project Timeline (based on kumo.lock):");
        println!(" - Created: {:?}", created);
        println!(" - Last Update: {:?}", modified);
        if let Ok(lockfile_str) = std::fs::read_to_string(&lock_path) {
            if let Ok(lockfile) = serde_yml::from_str::<Lockfile>(&lockfile_str) {
                println!(" - Dependencies: {}", lockfile.packages.len());
            }
        }
    } else {
        println!("No timeline available. Run 'kumo install' to generate a lockfile.");
    }
    Ok(())
}
