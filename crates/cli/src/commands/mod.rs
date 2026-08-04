use anyhow::Result;
use kumo_core::Store;
use resolver::Resolver;
use security::SecurityEngine;
use std::path::PathBuf;

/// Shared context passed to all command handlers.
pub struct CommandContext {
    pub store: Store,
    pub security: SecurityEngine,
    pub resolver: Resolver,
    pub config_path: Option<PathBuf>,
}

/// Trait implemented by all CLI commands for unified dispatch.
#[async_trait::async_trait(?Send)]
pub trait Command {
    async fn run(&self, ctx: &CommandContext) -> Result<()>;
}

macro_rules! register_commands {
    (
        $(
            $(#[$attr:meta])*
            $name:ident
        ),* $(,)?
    ) => {
        $(
            pub mod $name;
            paste::paste! {
                pub use $name::[<$name:camel Command>];
            }
        )*

        paste::paste! {
            #[derive(clap::Subcommand)]
            pub enum Commands {
                $(
                    $(#[$attr])*
                    [<$name:camel>]([<$name:camel Command>]),
                )*
                #[command(external_subcommand)]
                External(Vec<String>),
            }

            #[async_trait::async_trait(?Send)]
            impl Command for Commands {
                async fn run(&self, ctx: &CommandContext) -> anyhow::Result<()> {
                    match self {
                        $(
                            Commands::[<$name:camel>](cmd) => cmd.run(ctx).await,
                        )*
                        Commands::External(args) => {
                            if args.is_empty() {
                                anyhow::bail!("No script specified");
                            }
                            crate::commands::run::execute(&args[0], args[1..].to_vec()).await
                        }
                    }
                }
            }
        }
    }
}

register_commands!(
    #[command(
        alias = "i",
        about = "Install dependencies from kumo.json or package.json"
    )]
    install,
    #[command(alias = "a", about = "Add a new package to the project")]
    add,
    #[command(
        alias = "rm",
        alias = "un",
        alias = "uninstall",
        about = "Remove a package from the project"
    )]
    remove,
    #[command(about = "Scan project dependencies for known vulnerabilities")]
    scan,
    #[command(
        alias = "st",
        about = "Show statistics about the Kumo global cache and store"
    )]
    stats,
    #[command(about = "Maintenance commands to clean cached files or dependencies")]
    prune,
    #[command(
        alias = "dr",
        about = "Run a health check on the store to detect corrupted files"
    )]
    doctor,
    #[command(
        alias = "ex",
        about = "Explain why a package is present in the dependency tree"
    )]
    explain,
    #[command(about = "Detect and list local packages in a monorepo structure")]
    workspaces,
    #[command(about = "Extract a package to .kumo/patch for manual patching")]
    patch,
    #[command(about = "Show a security timeline for the project")]
    timeline,
    #[command(about = "Generate a Graphviz DOT file of the project's dependency tree")]
    graph,
    #[command(about = "Execute a script within the Kumo Sandbox for secure execution")]
    sandbox,
    #[command(about = "Check for and install the latest version of the Kumo CLI")]
    update,
    #[command(
        alias = "up",
        about = "Update project dependencies to their latest available versions"
    )]
    upgrade,
    #[command(about = "Manage Kumo configuration and security policies")]
    config,
    #[command(
        alias = "tsx",
        about = "Transpile, bundle, or execute TypeScript files natively"
    )]
    ts,
    #[command(about = "Symlink a local package into the project for development")]
    link,
    #[command(about = "Remove a symlinked package from the project")]
    unlink,
    #[command(about = "Automatically fix known vulnerabilities by upgrading affected packages")]
    audit_fix,
    #[command(about = "Manage Kumo Shield security state")]
    shield,
    #[command(about = "Unlock a Kumo Shield protected file for manual editing")]
    unlock,
    #[command(about = "Re-lock a previously unlocked file under Kumo Shield")]
    lock,
    #[command(about = "Run a script defined in package.json (interactive if no script specified)")]
    run,
    #[command(about = "Authenticate with the Kumo registry")]
    auth,
    #[command(about = "Manage dependencies (e.g. publish to registry)")]
    deps,
    #[command(about = "Initialize a new package.json file")]
    init,
    #[command(
        alias = "rt",
        about = "Manage Node.js runtime versions"
    )]
    runtime,
    #[command(about = "Run a secure CI pipeline (frozen lockfile, audit, ignore-scripts)")]
    ci,
    #[command(about = "Download and verify all tarballs to the global store without extracting")]
    fetch,
);
