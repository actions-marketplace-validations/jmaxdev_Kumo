use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Clone)]
pub enum TsSubcommand {
    #[command(about = "Run the TypeScript compiler (tsc). Docs: https://www.typescriptlang.org/docs/handbook/compiler-options.html")]
    Build {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long, help = "Bundle the output into a single file")]
        bundle: bool,
        #[arg(long, help = "Minify the output")]
        minify: bool,
        #[arg(long, help = "Custom output bundle name")]
        name: Option<String>,
        #[arg(long, default_value = "dist", help = "Output directory")]
        out: String,
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
            TsSubcommand::Build { args, bundle, minify, name, out } => {
                run_native_build(args, *bundle, *minify, name.as_deref(), out)?;
                Ok(())
            }
            TsSubcommand::Exec { args } => {
                if args.is_empty() {
                    anyhow::bail!("Usage: kumo ts exec <file.ts> [args...]");
                }
                let polyfill_url = crate::common::ensure_kumo_polyfills()?;
                let current_exe = std::env::current_exe()?;

                let mut cmd = std::process::Command::new("node");
                cmd.env("KUMO_BIN", current_exe);
                cmd.arg("--import").arg(format!("file://{}", polyfill_url));
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

                let _ = crate::common::update_kumo_dts();


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

fn run_native_build(args: &[String], bundle: bool, minify: bool, name: Option<&str>, out: &str) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let out_dir = if std::path::Path::new(out).is_absolute() {
        std::path::PathBuf::from(out)
    } else {
        current_dir.join(out)
    };

    if bundle {
        let entry_file = if !args.is_empty() {
            args[0].clone()
        } else if std::path::Path::new("index.ts").is_file() {
            "index.ts".to_string()
        } else if std::path::Path::new("src/index.ts").is_file() {
            "src/index.ts".to_string()
        } else {
            anyhow::bail!("No entry file specified and neither 'index.ts' nor 'src/index.ts' was found. Usage: kumo ts build <entry.ts> --bundle");
        };

        let entry_path = std::path::PathBuf::from(&entry_file);
        let abs_entry_path = if entry_path.is_absolute() {
            entry_path
        } else {
            current_dir.join(entry_path)
        };

        if !abs_entry_path.is_file() {
            anyhow::bail!("Entry file does not exist: {}", entry_file);
        }

        println!("Bundling {}...", entry_file);
        let bundle_code = run_native_bundle(&abs_entry_path, minify)?;

        let out_name = name.unwrap_or("bundle");
        let mut out_file = out_dir.join(out_name);
        if out_file.extension().is_none() {
            out_file.set_extension("js");
        }

        let _ = std::fs::create_dir_all(&out_dir);
        std::fs::write(&out_file, bundle_code)?;
        println!("Successfully bundled {} into {}", entry_file, out_file.display());
        return Ok(());
    }

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
                let mut out_file_path = out_dir.join(rel_path);
                out_file_path.set_extension("js");
                if let Some(parent) = out_file_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&out_file_path, js_code)?;
                println!("  Compiled: {} -> {}", rel_path.display(), out_file_path.strip_prefix(&current_dir).unwrap_or(&out_file_path).display());
                compiled_count += 1;
            }
            Err(e) => {
                eprintln!("  Error compiling {}: {}", rel_path.display(), e);
            }
        }
    }
    println!("Compilation finished. Compiled {} file(s) into output directory.", compiled_count);
    Ok(())
}

fn get_import_sources(program: &oxc_ast::ast::Program<'_>) -> Vec<String> {
    use oxc_ast::ast::Statement;
    let mut sources = Vec::new();
    for stmt in &program.body {
        match stmt {
            Statement::ImportDeclaration(import_decl) => {
                sources.push(import_decl.source.value.to_string());
            }
            Statement::ExportAllDeclaration(export_all_decl) => {
                sources.push(export_all_decl.source.value.to_string());
            }
            Statement::ExportNamedDeclaration(export_named_decl) => {
                if let Some(source) = &export_named_decl.source {
                    sources.push(source.value.to_string());
                }
            }
            _ => {}
        }
    }
    sources
}

fn resolve_import(current_file: &std::path::Path, specifier: &str) -> Option<std::path::PathBuf> {
    if !specifier.starts_with('.') && !specifier.starts_with('/') {
        return None;
    }
    let parent = current_file.parent()?;
    let path = parent.join(specifier);

    let candidates = [
        path.clone(),
        path.with_extension("ts"),
        path.with_extension("js"),
        path.join("index.ts"),
        path.join("index.js"),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn get_exports(program: &oxc_ast::ast::Program<'_>) -> (Vec<String>, bool) {
    use oxc_ast::ast::{Statement, Declaration, BindingPattern};
    let mut exported_names = Vec::new();
    let mut has_default = false;

    for stmt in &program.body {
        match stmt {
            Statement::ExportNamedDeclaration(export_decl) => {
                if let Some(declaration) = &export_decl.declaration {
                    match declaration {
                        Declaration::VariableDeclaration(var_decl) => {
                            for decl in &var_decl.declarations {
                                if let BindingPattern::BindingIdentifier(ident) = &decl.id {
                                    exported_names.push(ident.name.to_string());
                                }
                            }
                        }
                        Declaration::FunctionDeclaration(func_decl) => {
                            if let Some(ident) = &func_decl.id {
                                exported_names.push(ident.name.to_string());
                            }
                        }
                        Declaration::ClassDeclaration(class_decl) => {
                            if let Some(ident) = &class_decl.id {
                                exported_names.push(ident.name.to_string());
                            }
                        }
                        _ => {}
                    }
                }
                for specifier in &export_decl.specifiers {
                    exported_names.push(specifier.exported.name().to_string());
                }
            }
            Statement::ExportDefaultDeclaration(_) => {
                has_default = true;
            }
            _ => {}
        }
    }
    (exported_names, has_default)
}

fn run_native_bundle(entry_path: &std::path::Path, minify: bool) -> Result<String> {
    use std::collections::{HashSet, VecDeque};
    use std::fs;

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut modules_map = Vec::new();

    queue.push_back(entry_path.to_path_buf());

    while let Some(current_file) = queue.pop_front() {
        let abs_path = fs::canonicalize(&current_file)?;
        if visited.contains(&abs_path) {
            continue;
        }
        visited.insert(abs_path.clone());

        let source = fs::read_to_string(&abs_path)?;

        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        use oxc_span::SourceType;
        use oxc_semantic::SemanticBuilder;
        use oxc_transformer::{Transformer, TransformOptions};
        use oxc_codegen::{Codegen, CodegenOptions};

        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&abs_path)
            .map_err(|e| format!("Invalid source type: {:?}", e))
            .map_err(|e| anyhow::anyhow!(e))?;

        let parsed = Parser::new(&allocator, &source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            let errs: Vec<String> = parsed.diagnostics.into_iter().map(|e| format!("{:?}", e)).collect();
            anyhow::bail!("Compilation errors in {}:\n{}", abs_path.display(), errs.join("\n"));
        }

        let program = &parsed.program;

        let import_sources = get_import_sources(program);
        for specifier in &import_sources {
            if let Some(resolved) = resolve_import(&abs_path, specifier) {
                queue.push_back(resolved);
            }
        }

        let (exported_names, has_default) = get_exports(program);

        let mut program_mut = parsed.program;
        let semantic = SemanticBuilder::new()
            .build(&program_mut)
            .semantic;
        let options = TransformOptions::default();
        let _ = Transformer::new(&allocator, &abs_path, &options)
            .build_with_scoping(semantic.into_scoping(), &mut program_mut);

        let codegen_options = CodegenOptions {
            minify,
            ..CodegenOptions::default()
        };
        let transpiled_js = Codegen::new().with_options(codegen_options).build(&program_mut).code;

        let mut cjs_code = transpiled_js.clone();

        cjs_code = cjs_code.replace("export const ", "const ");
        cjs_code = cjs_code.replace("export let ", "let ");
        cjs_code = cjs_code.replace("export var ", "var ");
        cjs_code = cjs_code.replace("export function ", "function ");
        cjs_code = cjs_code.replace("export class ", "class ");
        cjs_code = cjs_code.replace("export default ", "const __kumo_default__ = ");

        let re_export_list = regex::Regex::new(r"export\s*\{[^}]*\}").unwrap();
        cjs_code = re_export_list.replace_all(&cjs_code, "").to_string();

        for name in exported_names {
            cjs_code.push_str(&format!("\n__kumo_exports__.{} = {};", name, name));
        }
        if has_default {
            cjs_code.push_str("\n__kumo_exports__.default = __kumo_default__;");
        }

        let re_named_import = regex::Regex::new(r#"import\s*\{\s*([^}]+)\s*\}\s*from\s*"([^"]+)";"#).unwrap();
        cjs_code = re_named_import.replace_all(&cjs_code, |caps: &regex::Captures| {
            let imports = &caps[1];
            let path = &caps[2];
            let resolved_imports = imports.split(',')
                .map(|imp| {
                    let imp = imp.trim();
                    if imp.contains(" as ") {
                        let parts: Vec<&str> = imp.split(" as ").collect();
                        format!("{}: {}", parts[0].trim(), parts[1].trim())
                    } else {
                        imp.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join(", ");
            format!("const {{ {} }} = __kumo_require__(\"{}\");", resolved_imports, path)
        }).to_string();

        let re_star_import = regex::Regex::new(r#"import\s*\*\s*as\s+(\w+)\s+from\s*"([^"]+)";"#).unwrap();
        cjs_code = re_star_import.replace_all(&cjs_code, r#"const $1 = __kumo_require__("$2");"#).to_string();

        let re_default_import = regex::Regex::new(r#"import\s+(\w+)\s+from\s*"([^"]+)";"#).unwrap();
        cjs_code = re_default_import.replace_all(&cjs_code, r#"const $1 = __kumo_require__("$2").default || __kumo_require__("$2");"#).to_string();

        let relative_id = if abs_path == fs::canonicalize(entry_path)? {
            "__entry__".to_string()
        } else {
            let entry_parent = entry_path.parent().unwrap_or(entry_path);
            let canonical_parent = fs::canonicalize(entry_parent)?;
            let rel = abs_path.strip_prefix(&canonical_parent).unwrap_or(&abs_path);
            format!("./{}", rel.to_string_lossy().replace('\\', "/"))
        };

        modules_map.push((relative_id, cjs_code));
    }

    let mut bundle = String::new();
    bundle.push_str("const __kumo_modules__ = {\n");
    for (id, code) in &modules_map {
        bundle.push_str(&format!("  \"{}\": function(__kumo_exports__, __kumo_require__, module) {{\n", id));
        for line in code.lines() {
            bundle.push_str(&format!("    {}\n", line));
        }
        bundle.push_str("  },\n");
    }
    bundle.push_str("};\n\n");

    bundle.push_str(r#"
const __kumo_cache__ = {};
function __kumo_require__(id, referrer = "__entry__") {
  let resolvedId = id;
  if (id.startsWith(".") || id.startsWith("/")) {
    if (referrer === "__entry__") {
      resolvedId = id;
    } else {
      let refParts = referrer.split("/");
      refParts.pop();
      let pathParts = id.split("/");
      for (let part of pathParts) {
        if (part === ".") continue;
        if (part === "..") {
          refParts.pop();
        } else {
          refParts.push(part);
        }
      }
      resolvedId = refParts.join("/");
    }

    let candidates = [
      resolvedId,
      resolvedId + ".ts",
      resolvedId + ".js",
      resolvedId + "/index.ts",
      resolvedId + "/index.js"
    ];
    let found = null;
    for (let cand of candidates) {
      if (__kumo_modules__[cand]) {
        found = cand;
        break;
      }
    }
    if (!found) {
      throw new Error(`Cannot find module '${id}' imported from '${referrer}'`);
    }
    resolvedId = found;
  } else {
    return require(id);
  }

  if (__kumo_cache__[resolvedId]) {
    return __kumo_cache__[resolvedId].exports;
  }

  const module = { exports: {} };
  __kumo_cache__[resolvedId] = module;

  const localRequire = (newId) => __kumo_require__(newId, resolvedId);
  __kumo_modules__[resolvedId](module.exports, localRequire, module);
  return module.exports;
}

__kumo_require__("__entry__");
"#);

    Ok(bundle)
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

    #[test]
    fn test_bundle_simple_files() {
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("kumo_test_{}", timestamp));
        let _ = std::fs::create_dir_all(&temp_dir);

        let lib_path = temp_dir.join("lib.ts");
        let main_path = temp_dir.join("main.ts");

        std::fs::write(&lib_path, "export const foo: number = 42;").unwrap();
        std::fs::write(&main_path, "import { foo } from './lib';\nconsole.log(foo);").unwrap();

        let bundle_code = run_native_bundle(&main_path, false).unwrap();
        assert!(bundle_code.contains("__kumo_modules__"));
        assert!(bundle_code.contains("\"__entry__\""));
        assert!(bundle_code.contains("const { foo } = __kumo_require__(\"./lib\");"));
        assert!(bundle_code.contains("const foo = 42;"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
