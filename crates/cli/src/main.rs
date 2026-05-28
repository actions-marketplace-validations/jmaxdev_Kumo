use anyhow::Result;
use clap::{Parser, Subcommand};


mod commands;

#[derive(Parser)]
#[command(name = "kumo")]
#[command(version)]
#[command(about = "A security-first, space-efficient package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "i")]
    #[command(about = "Install dependencies from kumo.json or package.json")]
    Install {
        #[arg(long)]
        log: bool,
    },
    #[command(alias = "a")]
    #[command(about = "Add a new package to the project")]
    Add {
        name: String,
        #[arg(short, long)]
        dev: bool,
        #[arg(short, long)]
        global: bool,
        #[arg(long)]
        log: bool,
    },
    #[command(alias = "rm")]
    #[command(alias = "un")]
    #[command(alias = "uninstall")]
    #[command(about = "Remove a package from the project")]
    Remove {
        name: String,
    },
    #[command(about = "Scan project dependencies for known vulnerabilities")]
    Scan,
    #[command(alias = "st")]
    #[command(about = "Show statistics about the Kumo global cache and store")]
    Stats,
    #[command(about = "Maintenance commands to clean cached files or dependencies")]
    Prune {
        #[command(subcommand)]
        subcommand: commands::prune::PruneSubcommand,
    },
    #[command(alias = "dr")]
    #[command(about = "Run a health check on the store to detect corrupted files")]
    Doctor,
    #[command(alias = "ex")]
    #[command(about = "Explain why a package is present in the dependency tree")]
    Explain {
        name: String,
    },
    #[command(about = "Manage Kumo configuration and security policies")]
    Config {
        #[command(subcommand)]
        subcommand: ConfigSubcommand,
    },
    #[command(about = "Detect and list local packages in a monorepo structure")]
    Workspaces,
    #[command(about = "Extract a package to .kumo/patch for manual patching")]
    Patch {
        name: String,
    },
    #[command(about = "Show a security timeline for the project")]
    Timeline,
    #[command(about = "Generate a Graphviz DOT file of the project's dependency tree")]
    Graph,
    #[command(about = "Execute a script within the Kumo Sandbox for secure execution")]
    Sandbox {
        script: String,
    },
    #[command(about = "Check for and install the latest version of the Kumo CLI")]
    Update {
        #[arg(long)]
        pre: bool,
        version: Option<String>,
    },
    #[command(alias = "up")]
    #[command(about = "Update project dependencies to their latest available versions")]
    Upgrade {
        packages: Vec<String>,
        #[arg(short = 'L', long)]
        latest: bool,
        #[arg(long)]
        prod: bool,
        #[arg(long)]
        dev: bool,
        #[arg(short = 'F', long)]
        fixed: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
        #[arg(long)]
        log: bool,
    },
    #[command(alias = "tsx")]
    #[command(about = "Execute TypeScript files via tsx or compile with tsc")]
    Ts {
        #[command(subcommand)]
        subcommand: TsSubcommand,
    },
    #[command(external_subcommand)]
    External(Vec<String>),
    #[command(about = "Symlink a local package into the project for development")]
    Link {
        path: String,
    },
    #[command(about = "Remove a symlinked package from the project")]
    Unlink {
        name: String,
    },
    #[command(alias = "audit-fix")]
    #[command(about = "Automatically fix known vulnerabilities by upgrading affected packages")]
    AuditFix,
    #[command(about = "Run a script defined in package.json (interactive if no script specified)")]
    Run {
        script: Option<String>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(about = "Manage Kumo Shield security state")]
    Shield {
        #[command(subcommand)]
        action: commands::shield::ShieldAction,
    },
    #[command(about = "Unlock a Kumo Shield protected file for manual editing")]
    Unlock {
        file: String,
    },
    #[command(about = "Re-lock a previously unlocked file under Kumo Shield")]
    Lock {
        file: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TsSubcommand {
    #[command(about = "Run the TypeScript compiler (tsc). Docs: https://www.typescriptlang.org/docs/handbook/compiler-options.html")]
    Build {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(about = "Execute a TypeScript file directly (tsx). Docs: https://tsx.hirok.io/getting-started")]
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(about = "Initialize a new TypeScript project (tsc --init)")]
    Init,
    #[command(about = "Type-check the project without emitting files (tsc --noEmit)")]
    Check {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigSubcommand {
    Init,
    Default {
        setting: String,
        value: String,
    },
}

mod common;

#[tokio::main]
async fn main() -> Result<()> {


    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            if tokio::signal::ctrl_c().await.is_ok() {
                std::process::exit(1);
            }
        }
    });

    let cli = Cli::parse();

    let update_check_handle = if !matches!(cli.command, Commands::Update { .. }) {
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

    match cli.command {
        Commands::Install { log } => {
            let config_path = config_path.ok_or_else(|| anyhow::anyhow!("Neither kumo.json nor package.json found in current directory"))?;
            commands::install::execute(&store, &resolver, &security, log, config_path).await?;
        }
        Commands::Add {
            name,
            dev,
            global,
            log,
        } => {
            commands::add::execute(&store, &resolver, &security, name, dev, global, log, config_path).await?;
        }
        Commands::Remove { name } => {
            commands::remove::execute(&store, &resolver, &security, name, config_path).await?;
        }
        Commands::Scan => {
            commands::scan::execute(&security).await?;
        }
        Commands::Stats => {
            commands::stats::execute(&store).await?;
        }
        Commands::Prune { subcommand } => {
            commands::prune::execute(&store, subcommand).await?;
        }
        Commands::Doctor => {
            commands::doctor::execute(&store).await?;
        }
        Commands::Explain { name } => {
            commands::explain::execute(&name).await?;
        }
        Commands::Workspaces => {
            commands::workspaces::execute().await?;
        }
        Commands::Patch { name } => {
            commands::patch::execute(name).await?;
        }
        Commands::Timeline => {
            commands::timeline::execute().await?;
        }
        Commands::Graph => {
            commands::graph::execute().await?;
        }
        Commands::Sandbox { script } => {
            commands::sandbox::execute(script).await?;
        }
        Commands::Update { pre, version } => commands::update::execute(pre, version).await?,
        Commands::Upgrade { packages, latest, prod, dev, fixed, dry_run, log } => {
            let config_path = config_path.ok_or_else(|| anyhow::anyhow!("Neither kumo.json nor package.json found in current directory"))?;
            commands::upgrade::execute(&store, &resolver, &security, packages, latest, prod, dev, fixed, dry_run, log, config_path).await?;
        }
        Commands::Config { subcommand } => {
            commands::config::execute(subcommand).await?;
        }
        Commands::Ts { subcommand } => {
            let current_exe = std::env::current_exe()?;
            let kx_exe = current_exe.with_file_name(if cfg!(windows) { "kx.exe" } else { "kx" });
            
            let mut cmd = std::process::Command::new(kx_exe);
            
            match subcommand {
                TsSubcommand::Build { args } => {
                    cmd.arg("-p").arg("typescript").arg("tsc").args(args);
                }
                TsSubcommand::Exec { args } => {
                    cmd.arg("tsx").args(args);
                }
                TsSubcommand::Init => {
                    cmd.arg("-p").arg("typescript").arg("tsc").arg("--init");
                }
                TsSubcommand::Check { args } => {
                    cmd.arg("-p").arg("typescript").arg("tsc").arg("--noEmit").args(args);
                }
            }
            
            let status = cmd.status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::External(args) => {
            if args.is_empty() {
                anyhow::bail!("No script specified");
            }
            let script_name = &args[0];
            let script_args = &args[1..];
            commands::run::execute(script_name, script_args.to_vec()).await?;
        }
        Commands::Link { path } => {
            commands::link::execute(path).await?;
        }
        Commands::Unlink { name } => {
            commands::unlink::execute(name).await?;
        }
        Commands::AuditFix => {
            commands::audit_fix::execute(&security).await?;
        }
        Commands::Run { script, args } => {
            if let Some(s) = script {
                commands::run::execute(&s, args).await?;
            } else {
                commands::run::execute_interactive().await?;
            }
        }
        Commands::Shield { action } => {
            commands::shield::execute(action).await?;
        }
        Commands::Unlock { file } => {
            commands::unlock::execute(file).await?;
        }
        Commands::Lock { file } => {
            commands::lock::execute(file).await?;
        }
    }

    if let Some(handle) = update_check_handle {
        if let Ok(Some(new_version)) = handle.await {
            common::print_update_banner(&new_version);
        }
    }

    Ok(())
}
