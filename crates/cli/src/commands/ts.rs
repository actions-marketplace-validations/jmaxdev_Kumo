use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Clone)]
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

#[derive(clap::Args)]
pub struct TsCommand {
    #[command(subcommand)]
    pub subcommand: TsSubcommand,
}

#[async_trait::async_trait(?Send)]
impl super::Command for TsCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> Result<()> {
        let current_exe = std::env::current_exe()?;
        let kx_exe = current_exe.with_file_name(if cfg!(windows) { "kx.exe" } else { "kx" });

        let mut cmd = std::process::Command::new(kx_exe);

        match &self.subcommand {
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
        Ok(())
    }
}
