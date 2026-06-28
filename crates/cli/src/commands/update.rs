use anyhow::Result;

#[derive(clap::Args)]
pub struct UpdateCommand {
    #[arg(long)]
    pub pre: bool,
    pub version: Option<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for UpdateCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.pre, self.version.clone()).await
    }
}

pub async fn execute(include_pre: bool, target_version: Option<String>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    if let Some(v) = &target_version {
        println!("Checking for version {}...", v);
    } else if include_pre {
        println!("Checking for latest pre-release...");
    } else {
        println!("Checking for latest stable release...");
    }

    let client = reqwest::Client::builder()
        .user_agent(kumo_core::config::DEFAULT_USER_AGENT)
        .build()?;

    let url = if include_pre || target_version.is_some() {
        kumo_core::config::GITHUB_RELEASES_LIST_URL
    } else {
        kumo_core::config::GITHUB_RELEASES_LATEST_URL
    };

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::NOT_FOUND && !include_pre {
            anyhow::bail!("No stable release found. Try 'kumo update --pre' to check for alpha/beta versions.");
        }
        anyhow::bail!("GitHub API error ({}). Please try again later.", response.status());
    }

    let release_val: serde_json::Value = response.json().await?;
    let release: serde_json::Value = if include_pre || target_version.is_some() {
        let releases = release_val.as_array()
            .ok_or_else(|| anyhow::anyhow!("Expected array of releases from GitHub API"))?;

        let mut best_match = None;
        let mut max_version = semver::Version::parse("0.0.0").unwrap();

        for rel in releases {
            if let Some(tag) = rel["tag_name"].as_str() {
                let version_str = tag.strip_prefix('v').unwrap_or(tag);

                if let Some(target) = &target_version {
                    let target_lower = target.to_lowercase();
                    if target_lower == "alpha" || target_lower == "beta" || target_lower == "rc" {
                        if version_str.contains(&target_lower) {
                            if let Ok(v) = semver::Version::parse(version_str) {
                                if v > max_version {
                                    max_version = v;
                                    best_match = Some(rel.clone());
                                }
                            }
                        }
                    } else if version_str == target || tag == target {
                        best_match = Some(rel.clone());
                        break;
                    }
                } else if let Ok(v) = semver::Version::parse(version_str) {
                    if v > max_version {
                        max_version = v;
                        best_match = Some(rel.clone());
                    }
                }
            }
        }
        best_match.ok_or_else(|| anyhow::anyhow!("No matching release found for: {}", target_version.as_deref().unwrap_or("latest")))?
    } else {
        release_val
    };

    if let Some(msg) = release.get("message").and_then(|m| m.as_str()) {
        anyhow::bail!("GitHub API Error: {}", msg);
    }

    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Could not find version information in the release."))?;
    let latest_version = latest_tag.trim_start_matches('v');

    if target_version.is_none() && latest_version == current_version {
        println!("Kumo is already up to date (v{})!", current_version);
        return Ok(());
    }

    println!(
        "A new version is available: v{} -> v{}",
        current_version, latest_version
    );

    #[cfg(target_os = "windows")]
    let asset_name = "kumo-windows.zip";
    #[cfg(target_os = "macos")]
    let asset_name = "kumo-macos.tar.gz";
    #[cfg(target_os = "linux")]
    let asset_name = "kumo-linux.tar.gz";

    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets found in release"))?;
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str().unwrap_or("").contains(asset_name))
        .ok_or_else(|| anyhow::anyhow!("Could not find asset for current OS: {}", asset_name))?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Asset download URL missing"))?;

    println!("Downloading update from {}...", download_url);
    let response = client.get(download_url).send().await?;
    let bytes = response.bytes().await?;

    let temp_dir = std::env::temp_dir().join("kumo_update");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    std::fs::create_dir_all(&temp_dir)?;

    let kumo_bin_name = if cfg!(target_os = "windows") { "kumo.exe" } else { "kumo" };
    let kx_bin_name = if cfg!(target_os = "windows") { "kx.exe" } else { "kx" };

    if asset_name.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let out_path = temp_dir.join(file.name());
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
        let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&temp_dir)?;
    };


    let mut found_kumo = None;
    let mut found_kx = None;

    fn find_binaries(dir: &std::path::Path, kumo_name: &str, kx_name: &str, kumo: &mut Option<std::path::PathBuf>, kx: &mut Option<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find_binaries(&path, kumo_name, kx_name, kumo, kx);
                } else if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name == kumo_name { *kumo = Some(path); }
                    else if name == kx_name { *kx = Some(path); }
                }
            }
        }
    }

    find_binaries(&temp_dir, kumo_bin_name, kx_bin_name, &mut found_kumo, &mut found_kx);

    let kumo_src = found_kumo.ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in update archive", kumo_bin_name))?;

    println!("Applying update for Kumo...");
    self_replace::self_replace(&kumo_src)?;

    if let Some(kx_src) = found_kx {
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(exe_dir) = current_exe.parent() {
                let kx_dest = exe_dir.join(kx_bin_name);
                if kx_dest.exists() {
                    println!("Applying update for KX...");
                    let temp_old = kx_dest.with_extension("old_kx");
                    let _ = std::fs::rename(&kx_dest, &temp_old);
                    if std::fs::copy(&kx_src, &kx_dest).is_ok() {
                        let _ = std::fs::remove_file(temp_old);
                    } else {
                        let _ = std::fs::rename(&temp_old, &kx_dest);
                        println!("Warning: Failed to update KX.");
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    println!("Successfully updated to v{}!", latest_version);
    Ok(())
}
