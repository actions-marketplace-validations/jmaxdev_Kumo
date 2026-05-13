use anyhow::Result;
use clap::Parser;
use std::process::Command;

#[derive(Parser)]
#[command(name = "kx")]
#[command(about = "Kumo Execute: Run binaries from dependencies/.bin or node_modules/.bin", long_about = None)]
struct KxCli {
    binary: String,
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = KxCli::parse();
    let (store, security, resolver) = common::init_components().await?;

    let bin_dir = std::env::current_dir()?
        .join(common::get_deps_dir())
        .join(".bin");
    let bin_path = bin_dir.join(&cli.binary);
    let bin_path_cmd = bin_dir.join(format!("{}.cmd", cli.binary));

    let exe = if bin_path_cmd.exists() {
        bin_path_cmd
    } else if bin_path.exists() {
        bin_path
    } else {
        println!("Package '{}' not found locally.", cli.binary);
        print!("Do you want to install and execute it using Kumo? (y/N): ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            let global_bin =
                install_temp_package(&store, &resolver, &security, &cli.binary).await?;
            global_bin.join(format!("{}.cmd", cli.binary))
        } else {
            anyhow::bail!("Execution cancelled.");
        }
    };

    let mut child = Command::new(&exe)
        .args(cli.args)
        .env(
            "PATH",
            format!(
                "{};{}",
                bin_dir.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .spawn()?;

    child.wait()?;
    Ok(())
}

async fn install_temp_package(
    store: &kumo_core::Store,
    resolver: &resolver::Resolver,
    _security: &security::SecurityEngine,
    name: &str,
) -> Result<std::path::PathBuf> {
    println!("Fetching {} from registry...", name);
    let metadata = resolver
        .clone()
        .resolve_package(name.to_string(), "latest".to_string())
        .await?;

    let response = reqwest::get(&metadata.dist.tarball).await?;
    let bytes = response.bytes().await?;

    let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;

    let temp_root = dirs::home_dir()
        .unwrap()
        .join(".kumo")
        .join("temp")
        .join(name);
    let bin_dir = temp_root.join("bin");
    let node_modules = temp_root.join("node_modules").join(name);

    tokio::fs::create_dir_all(&bin_dir).await?;
    kumo_core::package::link_package(store, &node_modules, &file_map).await?;

    if let Some(bin) = metadata.bin {
        match bin {
            serde_json::Value::String(path) => {
                create_shim(&bin_dir, name, &node_modules.join(path)).await?;
            }
            serde_json::Value::Object(map) => {
                for (cmd_name, path) in map {
                    if let Some(p) = path.as_str() {
                        create_shim(&bin_dir, &cmd_name, &node_modules.join(p)).await?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(bin_dir)
}

async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    let shim_path = bin_dir.join(format!("{}.cmd", name));
    let content = format!("@ECHO OFF\nnode \"{}\" %*", target.display());
    tokio::fs::write(shim_path, content).await?;
    Ok(())
}
