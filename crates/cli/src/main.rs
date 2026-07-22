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

    let cwd = std::env::current_dir().ok();
    let kumo_json_path = cwd.as_ref().map(|p| p.join("kumo.json"));
    let pkg_json_path = cwd.as_ref().map(|p| p.join("package.json"));
    let kumo_config_path = cwd.as_ref().map(|p| p.join(kumo_core::config::KUMO_CONFIG_JSON));

    let config_path = if kumo_json_path.as_ref().map_or(false, |p| p.exists()) {
        kumo_json_path
    } else if pkg_json_path.as_ref().map_or(false, |p| p.exists()) {
        pkg_json_path
    } else if kumo_config_path.as_ref().map_or(false, |p| p.exists()) {
        kumo_config_path
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
