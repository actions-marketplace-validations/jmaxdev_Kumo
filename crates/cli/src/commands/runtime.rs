use anyhow::{Context, Result};
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct RuntimeCommand {
    #[command(subcommand)]
    action: Option<RuntimeAction>,
}

#[derive(clap::Subcommand)]
enum RuntimeAction {
    /// List installed Node.js versions
    List,
    /// Install and switch to the specified Node.js version
    Use {
        /// Version specifier (latest, lts, codename, major, or exact version)
        version: String,
        /// Switch/install locally in the project instead of globally
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
    /// Remove an installed Node.js version
    Remove {
        /// Version to remove
        version: String,
        /// Remove local version instead of global version
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
}

#[async_trait::async_trait(?Send)]
impl super::Command for RuntimeCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        match &self.action {
            Some(RuntimeAction::List) => list_runtimes().await,
            Some(RuntimeAction::Use { version, local }) => use_runtime(version, *local).await,
            Some(RuntimeAction::Remove { version, local }) => remove_runtime(version, *local).await,
            None => list_runtimes().await,
        }
    }
}

async fn fetch_node_index() -> Result<Vec<serde_json::Value>> {
    let client = reqwest::Client::builder()
        .user_agent(kumo_core::config::DEFAULT_USER_AGENT)
        .build()?;

    let response = client
        .get(kumo_core::config::NODE_DIST_INDEX_URL)
        .send()
        .await
        .context("Failed to fetch Node.js release index")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch Node.js releases (HTTP {})",
            response.status()
        );
    }

    let releases: Vec<serde_json::Value> = response.json().await?;
    Ok(releases)
}

fn resolve_version(releases: &[serde_json::Value], specifier: &str) -> Result<String> {
    let spec_lower = specifier.to_lowercase();

    match spec_lower.as_str() {
        "latest" => {
            releases
                .first()
                .and_then(|r| r["version"].as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("No Node.js releases found"))
        }
        "lts" => {
            releases
                .iter()
                .find(|r| r["lts"].is_string())
                .and_then(|r| r["version"].as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow::anyhow!("No LTS release found"))
        }
        _ => {
            let codename_match = releases.iter().find(|r| {
                r["lts"]
                    .as_str()
                    .map(|s| s.to_lowercase() == spec_lower)
                    .unwrap_or(false)
            });
            if let Some(r) = codename_match {
                if let Some(v) = r["version"].as_str() {
                    return Ok(v.to_string());
                }
            }

            let version_with_v = if spec_lower.starts_with('v') {
                spec_lower.clone()
            } else {
                format!("v{}", spec_lower)
            };

            let exact_match = releases
                .iter()
                .find(|r| {
                    r["version"]
                        .as_str()
                        .map(|v| v.to_lowercase() == version_with_v)
                        .unwrap_or(false)
                });
            if let Some(r) = exact_match {
                if let Some(v) = r["version"].as_str() {
                    return Ok(v.to_string());
                }
            }

            let prefix = format!("v{}.", specifier);
            let major_match = releases
                .iter()
                .find(|r| {
                    r["version"]
                        .as_str()
                        .map(|v| v.starts_with(&prefix))
                        .unwrap_or(false)
                });
            if let Some(r) = major_match {
                if let Some(v) = r["version"].as_str() {
                    return Ok(v.to_string());
                }
            }

            if specifier.contains('.') && specifier.matches('.').count() == 1 {
                let prefix_mm = format!("v{}.", specifier);
                let minor_match = releases
                    .iter()
                    .find(|r| {
                        r["version"]
                            .as_str()
                            .map(|v| v.starts_with(&prefix_mm))
                            .unwrap_or(false)
                    });
                if let Some(r) = minor_match {
                    if let Some(v) = r["version"].as_str() {
                        return Ok(v.to_string());
                    }
                }
            }

            anyhow::bail!(
                "Could not resolve Node.js version '{}'. Use 'latest', 'lts', a codename, or a specific version number.",
                specifier
            )
        }
    }
}

fn get_global_runtimes_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home
        .join(kumo_core::config::KUMO_DIR_NAME)
        .join(kumo_core::config::RUNTIMES_DIR_NAME)
        .join(kumo_core::config::NODE_RUNTIME_DIR_NAME))
}

fn get_local_runtimes_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd
        .join(kumo_core::config::KUMO_DIR_NAME)
        .join(kumo_core::config::RUNTIMES_DIR_NAME)
        .join(kumo_core::config::NODE_RUNTIME_DIR_NAME))
}

fn get_global_bin_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home
        .join(kumo_core::config::KUMO_DIR_NAME)
        .join("bin"))
}

fn get_local_bin_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(kumo_core::config::KUMO_DIR_NAME).join("bin"))
}

fn get_node_binary_subpath() -> &'static str {
    if cfg!(target_os = "windows") {
        "node.exe"
    } else {
        "bin/node"
    }
}

fn get_platform_arch() -> Result<(&'static str, &'static str)> {
    let os = if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        anyhow::bail!("Unsupported architecture. Kumo runtime supports x64 and arm64.");
    };

    Ok((os, arch))
}

fn check_eol_version(version: &str) -> Result<bool> {
    let clean = version.strip_prefix('v').unwrap_or(version);
    let major: u64 = clean
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if major <= kumo_core::config::NODE_EOL_MAJOR_VERSION {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            anyhow::bail!(
                "Refusing to install EOL Node.js {} in non-interactive mode. \
                 EOL versions no longer receive security patches.",
                version
            );
        }

        println!(
            "\n\x1b[33m⚠  Node.js {} has reached End of Life (EOL).\x1b[0m",
            version
        );
        println!("   EOL versions no longer receive security patches and are considered unsafe.\n");

        let confirm = Confirm::new()
            .with_prompt("Are you sure you want to install this version?")
            .default(false)
            .interact()
            .context("Failed to read user confirmation")?;

        if !confirm {
            println!("\nOperation cancelled. Use \x1b[36mkumo runtime use lts\x1b[0m to install the latest LTS version.");
            return Ok(false);
        }
    }

    Ok(true)
}

fn create_node_shim(bin_dir: &std::path::Path, node_bin_path: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(bin_dir)?;

    if cfg!(target_os = "windows") {
        let shim_path = bin_dir.join("node.cmd");
        let content = format!("@ECHO OFF\n\"{}\" %*", node_bin_path.display());
        std::fs::write(&shim_path, content)?;
    } else {
        let shim_path = bin_dir.join("node");
        let content = format!(
            "#!/bin/sh\nexec \"{}\" \"$@\"",
            node_bin_path.display()
        );
        std::fs::write(&shim_path, &content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&shim_path, perms)?;
        }
    }

    Ok(())
}

async fn use_runtime(specifier: &str, local: bool) -> Result<()> {
    println!("Resolving Node.js version for '{}'...", specifier);

    let releases = fetch_node_index().await?;
    let version = resolve_version(&releases, specifier)?;

    if !check_eol_version(&version)? {
        return Ok(());
    }

    let runtimes_dir = if local {
        get_local_runtimes_dir()?
    } else {
        get_global_runtimes_dir()?
    };

    let version_dir = runtimes_dir.join(&version);

    if version_dir.exists() {
        println!("Node.js {} is already installed.", version);
        set_active_version(&runtimes_dir, &version)?;
        let bin_dir = if local { get_local_bin_dir()? } else { get_global_bin_dir()? };
        let node_bin = version_dir.join(get_node_binary_subpath());
        create_node_shim(&bin_dir, &node_bin)?;
        println!("Active version set to {}.", version);
        return Ok(());
    }

    let (os, arch) = get_platform_arch()?;
    let (archive_name, extension) = if os == "win" {
        (format!("node-{}-{}-{}", version, os, arch), "zip")
    } else {
        (format!("node-{}-{}-{}", version, os, arch), "tar.gz")
    };

    let download_url = format!(
        "{}/{}/{}.{}",
        kumo_core::config::NODE_DIST_URL,
        version,
        archive_name,
        extension
    );

    println!("Downloading Node.js {} ({}-{})...", version, os, arch);

    let client = reqwest::Client::builder()
        .user_agent(kumo_core::config::DEFAULT_USER_AGENT)
        .build()?;

    let response = client
        .get(&download_url)
        .send()
        .await
        .context("Failed to download Node.js")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to download Node.js {} (HTTP {}). The version may not exist for your platform.",
            version,
            response.status()
        );
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█▓░"),
    );

    let mut bytes = Vec::with_capacity(total_size as usize);
    let mut stream = response.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error during download")?;
        pb.inc(chunk.len() as u64);
        bytes.extend_from_slice(&chunk);
    }
    pb.finish_with_message("Download complete");

    println!("Extracting...");
    let temp_dir = std::env::temp_dir().join(format!("kumo_node_{}", version));
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    std::fs::create_dir_all(&temp_dir)?;

    if extension == "zip" {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes))?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let out_path = temp_dir.join(file.mangled_name());
            if file.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(p) = out_path.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut out_file = std::fs::File::create(&out_path)?;
                std::io::copy(&mut file, &mut out_file)?;
            }
        }
    } else {
        let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(&bytes));
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&temp_dir)?;
    }

    std::fs::create_dir_all(&version_dir)?;

    let extracted_dir = temp_dir.join(&archive_name);
    if extracted_dir.exists() {
        move_dir_contents(&extracted_dir, &version_dir)?;
    } else {
        let mut entries = std::fs::read_dir(&temp_dir)?;
        if let Some(Ok(entry)) = entries.next() {
            if entry.path().is_dir() {
                move_dir_contents(&entry.path(), &version_dir)?;
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    let node_bin = version_dir.join(get_node_binary_subpath());
    if !node_bin.exists() {
        let _ = std::fs::remove_dir_all(&version_dir);
        anyhow::bail!(
            "Installation failed: Node.js binary not found at expected path. \
             Please report this issue."
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&node_bin)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&node_bin, perms)?;
    }

    set_active_version(&runtimes_dir, &version)?;
    let bin_dir = if local { get_local_bin_dir()? } else { get_global_bin_dir()? };
    create_node_shim(&bin_dir, &node_bin)?;

    let scope = if local { "locally" } else { "globally" };
    println!(
        "\n\x1b[32m✓\x1b[0m Node.js {} installed {} successfully!",
        version, scope
    );

    if !local {
        let bin = get_global_bin_dir()?;
        let old_path = std::env::var("PATH").unwrap_or_default();
        let bin_str = bin.to_string_lossy();
        if !old_path.contains(bin_str.as_ref()) {
            println!(
                "\n\x1b[33mTip:\x1b[0m Make sure \x1b[36m{}\x1b[0m is in your PATH.",
                bin.display()
            );
        }
    }

    Ok(())
}

fn move_dir_contents(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dst.join(entry.file_name());
        if entry.path().is_dir() {
            move_dir_contents(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn set_active_version(runtimes_dir: &std::path::Path, version: &str) -> Result<()> {
    std::fs::create_dir_all(runtimes_dir)?;
    let active_file = runtimes_dir.join(kumo_core::config::RUNTIME_ACTIVE_FILE);
    std::fs::write(&active_file, version)?;
    Ok(())
}

fn get_active_version(runtimes_dir: &std::path::Path) -> Option<String> {
    let active_file = runtimes_dir.join(kumo_core::config::RUNTIME_ACTIVE_FILE);
    std::fs::read_to_string(&active_file)
        .ok()
        .map(|s| s.trim().to_string())
}

async fn list_runtimes() -> Result<()> {
    let runtimes_dir = get_global_runtimes_dir()?;

    if !runtimes_dir.exists() {
        println!("No Node.js versions installed.");
        println!("Run \x1b[36mkumo runtime use latest\x1b[0m to install one.");
        return Ok(());
    }

    let active = get_active_version(&runtimes_dir);
    let mut versions: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&runtimes_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('v') {
                versions.push(name);
            }
        }
    }

    if versions.is_empty() {
        println!("No Node.js versions installed.");
        println!("Run \x1b[36mkumo runtime use latest\x1b[0m to install one.");
        return Ok(());
    }

    versions.sort_by(|a, b| {
        let parse = |v: &str| -> semver::Version {
            semver::Version::parse(v.strip_prefix('v').unwrap_or(v)).unwrap_or(semver::Version::new(0, 0, 0))
        };
        parse(b).cmp(&parse(a))
    });

    println!("Installed Node.js versions:\n");
    for version in &versions {
        let is_active = active.as_deref() == Some(version.as_str());
        if is_active {
            println!("  \x1b[32m→ {}\x1b[0m  (active)", version);
        } else {
            println!("    {}", version);
        }
    }
    println!();

    if let Ok(local_dir) = get_local_runtimes_dir() {
        if local_dir.exists() {
            let local_active = get_active_version(&local_dir);
            let mut local_versions: Vec<String> = Vec::new();

            for entry in std::fs::read_dir(&local_dir)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('v') {
                        local_versions.push(name);
                    }
                }
            }

            if !local_versions.is_empty() {
                local_versions.sort();
                println!("Local versions (this project):\n");
                for version in &local_versions {
                    let is_active = local_active.as_deref() == Some(version.as_str());
                    if is_active {
                        println!("  \x1b[32m→ {}\x1b[0m  (active)", version);
                    } else {
                        println!("    {}", version);
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

async fn remove_runtime(version_spec: &str, local: bool) -> Result<()> {
    let runtimes_dir = if local {
        get_local_runtimes_dir()?
    } else {
        get_global_runtimes_dir()?
    };

    let target_version = if version_spec.starts_with('v') {
        version_spec.to_string()
    } else {
        format!("v{}", version_spec)
    };

    let version_dir = runtimes_dir.join(&target_version);
    if !version_dir.exists() {
        anyhow::bail!("Node.js {} is not installed.", target_version);
    }

    std::fs::remove_dir_all(&version_dir)?;
    println!("\x1b[32m✓\x1b[0m Node.js {} removed.", target_version);

    if let Some(active) = get_active_version(&runtimes_dir) {
        if active == target_version {
            let active_file = runtimes_dir.join(kumo_core::config::RUNTIME_ACTIVE_FILE);
            let _ = std::fs::remove_file(&active_file);

            let bin_dir = if local { get_local_bin_dir()? } else { get_global_bin_dir()? };
            if cfg!(target_os = "windows") {
                let _ = std::fs::remove_file(bin_dir.join("node.cmd"));
            } else {
                let _ = std::fs::remove_file(bin_dir.join("node"));
            }

            println!("Active Node.js version has been cleared.");
            println!("Use \x1b[36mkumo runtime use <version>\x1b[0m to activate another version.");
        }
    }

    Ok(())
}
