use anyhow::Result;

#[derive(clap::Args)]
pub struct RunCommand {
    #[arg(long)]
    pub filter: Option<String>,
    pub script: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for RunCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        if let Some(ref filter_spec) = self.filter {
            std::env::set_var("KUMO_FILTER", filter_spec);
        }
        if let Some(s) = &self.script {
            execute(s, self.args.clone()).await
        } else {
            execute_interactive().await
        }
    }
}

pub async fn execute(name: &str, args: Vec<String>) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let config_path = project_dir.join("kumo.config.json");
    let mut inputs = vec![];
    let mut outputs = vec![];
    let mut use_cache = false;
    let mut found_custom_cache = false;

    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(cache_cfg) = v.get("cache").and_then(|c| c.get(name)) {
                    use_cache = true;
                    found_custom_cache = true;
                    if let Some(ins) = cache_cfg.get("inputs").and_then(|i| i.as_array()) {
                        inputs = ins.iter().filter_map(|i| i.as_str().map(|s| s.to_string())).collect();
                    }
                    if let Some(outs) = cache_cfg.get("outputs").and_then(|o| o.as_array()) {
                        outputs = outs.iter().filter_map(|o| o.as_str().map(|s| s.to_string())).collect();
                    }
                }
            }
        }
    }


    if !found_custom_cache {
        if name == "build" {
            use_cache = true;
            inputs = vec![

                "src/**/*.ts".to_string(),
                "src/**/*.tsx".to_string(),
                "src/**/*.js".to_string(),
                "src/**/*.jsx".to_string(),
                "src/**/*.cjs".to_string(),
                "src/**/*.mjs".to_string(),


                "*.ts".to_string(),
                "*.tsx".to_string(),
                "*.js".to_string(),
                "*.jsx".to_string(),
                "*.cjs".to_string(),
                "*.mjs".to_string(),
                "lib/**/*.ts".to_string(),
                "lib/**/*.js".to_string(),
                "lib/**/*.cjs".to_string(),
                "lib/**/*.mjs".to_string(),


                "app/**/*.ts".to_string(),
                "app/**/*.tsx".to_string(),
                "app/**/*.js".to_string(),
                "app/**/*.jsx".to_string(),
                "pages/**/*.ts".to_string(),
                "pages/**/*.tsx".to_string(),
                "pages/**/*.js".to_string(),
                "pages/**/*.jsx".to_string(),
                "components/**/*.ts".to_string(),
                "components/**/*.tsx".to_string(),
                "components/**/*.js".to_string(),
                "components/**/*.jsx".to_string(),


                "package.json".to_string(),
                "tsconfig.json".to_string(),
                "vite.config.ts".to_string(),
                "vite.config.js".to_string(),
                "next.config.js".to_string(),
                "next.config.mjs".to_string(),
                "next.config.ts".to_string(),
                "tailwind.config.js".to_string(),
                "tailwind.config.ts".to_string(),
                "postcss.config.js".to_string(),
                "postcss.config.mjs".to_string(),
            ];
            outputs = vec!["dist".to_string(), "build".to_string(), ".next".to_string()];
        }
    }

    let config_files = ["package.json", "kumo.json"];

    for config_file in config_files {
        let path = project_dir.join(config_file);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            if let Some(script_cmd) = v["scripts"][name].as_str() {
                let cache_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".kumo").join("cache").join("scripts");
                let mut hash_val = String::new();

                if use_cache {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(script_cmd.as_bytes());
                    if let Ok(lock) = std::fs::read_to_string(project_dir.join("kumo.lock")) {
                        hasher.update(lock.as_bytes());
                    }
                    let mut matched_files = vec![];
                    for pattern in &inputs {
                        if let Ok(entries) = glob::glob(pattern) {
                            for entry in entries.flatten() {
                                if entry.is_file() {
                                    matched_files.push(entry);
                                }
                            }
                        }
                    }
                    matched_files.sort();
                    for file in matched_files {
                        hasher.update(file.to_string_lossy().as_bytes());
                        if let Ok(data) = std::fs::read(&file) {
                            hasher.update(&data);
                        }
                    }
                    hash_val = hasher.finalize().to_hex().to_string();
                    let specific_cache = cache_dir.join(&hash_val);
                    if specific_cache.exists() {
                        println!("⚡ \x1b[32mKumo Cache Hit:\x1b[0m {} [{}]", name, &hash_val[0..8]);

                        if let Ok(out_log) = std::fs::read(specific_cache.join("stdout.log")) {
                            use std::io::Write;
                            let _ = std::io::stdout().write_all(&out_log);
                            let _ = std::io::stdout().flush();
                        }
                        if let Ok(err_log) = std::fs::read(specific_cache.join("stderr.log")) {
                            use std::io::Write;
                            let _ = std::io::stderr().write_all(&err_log);
                            let _ = std::io::stderr().flush();
                        }

                        for out_pattern in &outputs {
                            let clean_pattern = out_pattern.trim_end_matches("/**").trim_end_matches("/*").trim_end_matches('/');
                            let out_path = specific_cache.join(clean_pattern);
                            let target_path = project_dir.join(clean_pattern);
                            if out_path.exists() {
                                if out_path.is_dir() {
                                    let _ = crate::common::copy_dir_recursive(&out_path, &target_path).await;
                                } else {
                                    if let Some(p) = target_path.parent() {
                                        let _ = std::fs::create_dir_all(p);
                                    }
                                    let _ = std::fs::copy(&out_path, &target_path);
                                }
                            }
                        }
                        return Ok(());
                    }
                }

                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = std::process::Command::new("cmd");
                    c.arg("/C").arg(script_cmd);
                    c
                } else {
                    let mut c = std::process::Command::new("sh");
                    c.arg("-c").arg(script_cmd);
                    c
                };

                for arg in &args {
                    cmd.arg(arg);
                }

                let deps_dir = project_dir.join(crate::common::get_deps_dir());
                let bin_dir = deps_dir.join(".bin");
                if bin_dir.exists() {
                    let new_path = crate::common::prepend_to_path(&bin_dir);
                    cmd.env("PATH", new_path);
                }

                if let Ok(polyfill_url) = crate::common::ensure_kumo_polyfills() {
                    let old_node_opts = std::env::var("NODE_OPTIONS").unwrap_or_default();
                    let new_node_opts = if old_node_opts.is_empty() {
                        format!("--import file://{}", polyfill_url)
                    } else {
                        format!("--import file://{} {}", polyfill_url, old_node_opts)
                    };
                    cmd.env("NODE_OPTIONS", new_node_opts);
                }

                if use_cache {
                    let mut child = cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn()?;
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();

                    let mut stdout_log = Vec::new();
                    let mut stderr_log = Vec::new();

                    let mut out_reader = std::io::BufReader::new(stdout);
                    let mut err_reader = std::io::BufReader::new(stderr);

                    let mut stdout_writer = std::io::stdout();
                    let mut stderr_writer = std::io::stderr();

                    let out_handle = std::thread::spawn(move || {
                        use std::io::Read;
                        let mut buf = [0; 1024];
                        while let Ok(n) = out_reader.read(&mut buf) {
                            if n == 0 { break; }
                            stdout_log.extend_from_slice(&buf[..n]);
                            use std::io::Write;
                            let _ = stdout_writer.write_all(&buf[..n]);
                        }
                        stdout_log
                    });

                    let err_handle = std::thread::spawn(move || {
                        use std::io::Read;
                        let mut buf = [0; 1024];
                        while let Ok(n) = err_reader.read(&mut buf) {
                            if n == 0 { break; }
                            stderr_log.extend_from_slice(&buf[..n]);
                            use std::io::Write;
                            let _ = stderr_writer.write_all(&buf[..n]);
                        }
                        stderr_log
                    });

                    let status = child.wait()?;
                    let stdout_res = out_handle.join().unwrap();
                    let stderr_res = err_handle.join().unwrap();

                    if status.success() {
                        let specific_cache = cache_dir.join(&hash_val);
                        let _ = std::fs::create_dir_all(&specific_cache);
                        let _ = std::fs::write(specific_cache.join("stdout.log"), stdout_res);
                        let _ = std::fs::write(specific_cache.join("stderr.log"), stderr_res);

                        for out_pattern in &outputs {
                            let clean_pattern = out_pattern.trim_end_matches("/**").trim_end_matches("/*").trim_end_matches('/');
                            let target_path = project_dir.join(clean_pattern);
                            let cache_path = specific_cache.join(clean_pattern);
                            if target_path.exists() {
                                if target_path.is_dir() {
                                    let _ = crate::common::copy_dir_recursive(&target_path, &cache_path).await;
                                } else {
                                    if let Some(p) = cache_path.parent() {
                                        let _ = std::fs::create_dir_all(p);
                                    }
                                    let _ = std::fs::copy(&target_path, &cache_path);
                                }
                            }
                        }
                    } else {
                        anyhow::bail!("Script '{}' failed with status: {}", name, status);
                    }
                } else {
                    let status = cmd.status()?;
                    if !status.success() {
                        anyhow::bail!("Script '{}' failed with status: {}", name, status);
                    }
                }

                return Ok(());
            }
        }
    }

    let deps_dir = project_dir.join(crate::common::get_deps_dir());
    let bin_path = deps_dir.join(".bin").join(name);
    let bin_path_cmd = deps_dir.join(".bin").join(format!("{}.cmd", name));

    let actual_bin = if bin_path.exists() {
        Some(bin_path)
    } else if bin_path_cmd.exists() {
        Some(bin_path_cmd)
    } else {
        None
    };

    if let Some(bin) = actual_bin {
        let mut cmd = std::process::Command::new(bin);
        for arg in args {
            cmd.arg(arg);
        }

        let bin_dir = deps_dir.join(".bin");
        let new_path = crate::common::prepend_to_path(&bin_dir);
        cmd.env("PATH", new_path);

        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("Binary '{}' failed with status: {}", name, status);
        }
        return Ok(());
    }

    let fallback_global_bin = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".kumo").join("bin").join(name);
    let fallback_global_bin_cmd = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".kumo").join("bin").join(format!("{}.cmd", name));

    let actual_global = if fallback_global_bin.exists() {
        Some(fallback_global_bin)
    } else if fallback_global_bin_cmd.exists() {
        Some(fallback_global_bin_cmd)
    } else {
        None
    };

    if let Some(bin) = actual_global {
        let mut cmd = std::process::Command::new(bin);
        for arg in args {
            cmd.arg(arg);
        }
        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("Global binary '{}' failed with status: {}", name, status);
        }
        return Ok(());
    }

    anyhow::bail!("Script or binary '{}' not found in configuration or .bin", name);
}

pub async fn execute_interactive() -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Select};

    let project_dir = std::env::current_dir()?;
    let config_files = ["package.json", "kumo.json"];
    let mut scripts = std::collections::BTreeMap::new();

    for config_file in config_files {
        let path = project_dir.join(config_file);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(s) = v.get("scripts").and_then(|s| s.as_object()) {
                    for (k, val) in s {
                        if let Some(cmd) = val.as_str() {
                            scripts.insert(k.clone(), cmd.to_string());
                        }
                    }
                }
            }
        }
    }

    if scripts.is_empty() {
        anyhow::bail!("No scripts found in package.json or kumo.json");
    }

    let mut items = Vec::new();
    let mut keys = Vec::new();

    for (k, v) in &scripts {
        items.push(format!("{:<15} {}", k, v));
        keys.push(k.clone());
    }

    println!("Select a script to run:");
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact()?;

    let selected_script = &keys[selection];
    println!("Running script '{}'...", selected_script);

    let args = Vec::new();
    execute(selected_script, args).await
}
