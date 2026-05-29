use anyhow::Result;
use clap::Parser;

mod commands;
pub mod common;

use commands::{Command, CommandContext, Commands};

#[derive(Parser)]
#[command(name = "kumo")]
#[command(version)]
#[command(about = "A security-first, space-efficient package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    tokio::spawn(async move {
        if let Ok(_) = tokio::signal::ctrl_c().await {
            println!("\nOperation cancelled by user. Cleaning up...");
            std::process::exit(130);
        }
    });

    let cli = Cli::parse();

    let update_check_handle = if !matches!(cli.command, Commands::Update(_)) {
        Some(tokio::spawn(common::check_for_new_version()))
    } else {
        None
    };

    let (store, security, resolver) = common::init_components().await?;

    let kumo_json_path = std::env::current_dir()?.join("kumo.json");
    let pkg_json_path = std::env::current_dir()?.join("package.json");
    let config_path = if kumo_json_path.exists() {
        Some(kumo_json_path)
    } else if pkg_json_path.exists() {
        Some(pkg_json_path)
    } else {
        None
    };

    let ctx = CommandContext {
        store,
        security,
        resolver,
        config_path,
    };

    cli.command.run(&ctx).await?;

    if let Some(handle) = update_check_handle {
        if let Ok(Some(new_version)) = handle.await {
            common::print_update_banner(&new_version);
        }
    }

    Ok(())
}
