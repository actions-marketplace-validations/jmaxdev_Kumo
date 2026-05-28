use anyhow::Result;

pub async fn execute(script: String) -> Result<()> {
    println!("Executing '{}' in Kumo Sandbox...", script);
    let mut command = security::sandbox::SandboxRunner::create_command(&std::env::current_dir()?, &script, false, None);
    let status = security::sandbox::SandboxRunner::execute_command(&mut command)?;
    if !status.success() {
        anyhow::bail!("Sandbox execution failed");
    }
    Ok(())
}
