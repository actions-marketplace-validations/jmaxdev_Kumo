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
    #[command(hide = true)]
    Transpile {
        file: String,
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
        match &self.subcommand {
            TsSubcommand::Build { args } => {
                run_native_build(args)?;
                Ok(())
            }
            TsSubcommand::Exec { args } => {
                if args.is_empty() {
                    anyhow::bail!("Usage: kumo ts exec <file.ts> [args...]");
                }
                let _polyfill_url = crate::common::ensure_kumo_polyfills()?;
                let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
                let loader_path = home.join(".kumo").join("lib").join("loader.mjs");
                let current_exe = std::env::current_exe()?;

                let mut cmd = std::process::Command::new("node");
                cmd.env("KUMO_BIN", current_exe);
                cmd.arg("--import").arg(format!("file:///{}", loader_path.to_string_lossy().replace('\\', "/")));
                cmd.args(args);

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

                Ok(())
            }
            TsSubcommand::Check { args: _ } => {
                anyhow::bail!("Native Rust-based type checking is not supported. Use the official 'tsc --noEmit' tool for type checking.");
            }
            TsSubcommand::Transpile { file } => {
                let source = std::fs::read_to_string(file)?;
                let output = transpile_code(&source, file)
                    .map_err(|e| anyhow::anyhow!("Transpilation error: {}", e))?;
                print!("{}", output);
                Ok(())
            }
        }
    }
}

fn transpile_code(source: &str, filename: &str) -> std::result::Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use oxc_semantic::SemanticBuilder;
    use oxc_transformer::{Transformer, TransformOptions};
    use oxc_codegen::Codegen;
    use std::path::Path;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename)
        .map_err(|e| format!("Invalid source type: {:?}", e))?;

    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        let errs: Vec<String> = parsed.diagnostics.into_iter().map(|e| format!("{:?}", e)).collect();
        return Err(errs.join("\n"));
    }

    let mut program = parsed.program;

    let semantic = SemanticBuilder::new()
        .build(&program)
        .semantic;

    let options = TransformOptions::default();
    
    let _ = Transformer::new(&allocator, Path::new(filename), &options)
        .build_with_scoping(semantic.into_scoping(), &mut program);

    let code = Codegen::new().build(&program).code;
    Ok(code)
}

fn run_native_build(args: &[String]) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let mut files = Vec::new();
    
    fn visit_dirs(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name != "node_modules" && !name.starts_with('.') {
                        visit_dirs(&path, files)?;
                    }
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext == "ts" && !path.to_string_lossy().ends_with(".d.ts") {
                            files.push(path);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    if args.is_empty() {
        println!("Compiling TypeScript files natively in current directory...");
        visit_dirs(&current_dir, &mut files)?;
    } else {
        println!("Compiling specified TypeScript files/directories natively...");
        for arg in args {
            let path = std::path::PathBuf::from(arg);
            let abs_path = if path.is_absolute() {
                path
            } else {
                current_dir.join(path)
            };

            if abs_path.is_file() {
                if let Some(ext) = abs_path.extension().and_then(|s| s.to_str()) {
                    if ext == "ts" && !abs_path.to_string_lossy().ends_with(".d.ts") {
                        files.push(abs_path);
                    }
                }
            } else if abs_path.is_dir() {
                visit_dirs(&abs_path, &mut files)?;
            } else {
                eprintln!("Warning: Path does not exist or is not supported: {}", arg);
            }
        }
    }
    
    let mut compiled_count = 0;
    for file_path in files {
        let rel_path = file_path.strip_prefix(&current_dir).unwrap_or(&file_path);
        let source = std::fs::read_to_string(&file_path)?;
        match transpile_code(&source, file_path.to_str().unwrap_or("file.ts")) {
            Ok(js_code) => {
                let js_path = file_path.with_extension("js");
                std::fs::write(&js_path, js_code)?;
                println!("  Compiled: {}", rel_path.display());
                compiled_count += 1;
            }
            Err(e) => {
                eprintln!("  Error compiling {}: {}", rel_path.display(), e);
            }
        }
    }
    println!("Compilation finished. Compiled {} file(s).", compiled_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpile_code_strips_types() {
        let ts_code = "const x: number = 10; function greet(name: string): void { console.log('hello ' + name); }";
        let js_code = transpile_code(ts_code, "test.ts").unwrap();
        assert!(js_code.contains("const x = 10;"));
        assert!(js_code.contains("function greet(name)"));
        assert!(!js_code.contains(": number"));
        assert!(!js_code.contains(": string"));
        assert!(!js_code.contains(": void"));
    }
}
