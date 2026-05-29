use anyhow::Result;

#[derive(clap::Args)]
pub struct SandboxCommand {
    pub script: String,
}

#[async_trait::async_trait(?Send)]
impl super::Command for SandboxCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.script.clone()).await
    }
}

pub async fn execute(script: String) -> Result<()> {
    println!("Executing '{}' in Kumo Sandbox...", script);
    let mut command = security::sandbox::SandboxRunner::create_command(&std::env::current_dir()?, &script, false, None);
    let status = security::sandbox::SandboxRunner::execute_command(&mut command)?;
    if !status.success() {
        anyhow::bail!("Sandbox execution failed");
    }
    Ok(())
}
