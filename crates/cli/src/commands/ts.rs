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
                let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                let kumo_dir = home.join(".kumo");
                let lib_dir = kumo_dir.join("lib");
                if !lib_dir.exists() {
                    let _ = std::fs::create_dir_all(&lib_dir);
                }
                
                let polyfill_path = lib_dir.join("api.mjs");
                let polyfill_content = include_str!("../lib/api.mjs").replace("__KUMO_VERSION__", env!("CARGO_PKG_VERSION"));
                let _ = std::fs::write(&polyfill_path, polyfill_content);

                let dts_path = lib_dir.join("kumo.d.ts");
                let dts_content = include_str!("../lib/kumo.d.ts");
                let _ = std::fs::write(&dts_path, dts_content);

                cmd.arg("tsx");
                
                if polyfill_path.exists() {
                    let mut polyfill_url = polyfill_path.to_string_lossy().replace('\\', "/");
                    if cfg!(windows) && !polyfill_url.starts_with('/') {
                        polyfill_url = format!("/{}", polyfill_url);
                    }
                    cmd.arg("--import").arg(format!("file://{}", polyfill_url));
                }
                
                cmd.args(args);
            }
            TsSubcommand::Init => {
                let current_dir = std::env::current_dir()?;
                let dot_kumo_dir = current_dir.join(".kumo");
                if !dot_kumo_dir.exists() {
                    let _ = std::fs::create_dir_all(&dot_kumo_dir);
                }

                let dts_path = dot_kumo_dir.join("kumo.d.ts");
                let dts_content = include_str!("../lib/kumo.d.ts");
                let _ = std::fs::write(&dts_path, dts_content);

                let tsconfig_path = current_dir.join("tsconfig.json");
                if !tsconfig_path.exists() {
                    let tsconfig_content = r#"{
  "compilerOptions": {
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["**/*.ts", ".kumo/**/*.d.ts"]
}"#;
                    let _ = std::fs::write(&tsconfig_path, tsconfig_content);
                    println!("Initialized Kumo TypeScript project configuration.");
                } else {
                    println!("tsconfig.json already exists. Updated Kumo types in .kumo/");
                }
                
                let pkg_json_path = current_dir.join("package.json");
                if !pkg_json_path.exists() {
                    let pkg_json_content = r#"{
  "type": "module"
}"#;
                    let _ = std::fs::write(&pkg_json_path, pkg_json_content);
                }

                return Ok(());
            }
            TsSubcommand::Check { args } => {
                cmd.arg("-p").arg("typescript").arg("tsc").arg("--noEmit").args(args);
            }
        }

        #[cfg(windows)]
        {
            extern "system" {
                fn SetConsoleCtrlHandler(handler: usize, add: i32) -> i32;
            }
            unsafe { SetConsoleCtrlHandler(0, 1); }
        }

        let mut child = cmd.spawn()?;
        let status = child.wait()?;

        #[cfg(windows)]
        {
            extern "system" {
                fn SetConsoleCtrlHandler(handler: usize, add: i32) -> i32;
            }
            unsafe { SetConsoleCtrlHandler(0, 0); }
        }

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}
