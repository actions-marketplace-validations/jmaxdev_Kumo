use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use dialoguer::Input;
use kumo_core::credentials;
use kumo_core::keys;

#[derive(Subcommand, Clone)]
pub enum DepsSubcommand {
    #[command(about = "Publish a package to the registry")]
    Publish {
        #[arg(default_value = ".")]
        path: String,

        #[arg(long, help = "Custom registry URL to publish to")]
        registry: Option<String>,
    },
}

#[derive(Args, Clone)]
pub struct DepsCommand {
    #[command(subcommand)]
    pub subcommand: DepsSubcommand,
}

#[async_trait::async_trait(?Send)]
impl super::Command for DepsCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        match &self.subcommand {
            DepsSubcommand::Publish { path, registry } => {
                execute_publish(ctx, path, registry.as_deref()).await
            }
        }
    }
}

pub async fn execute_publish(
    ctx: &super::CommandContext,
    path: &str,
    registry_opt: Option<&str>,
) -> Result<()> {
    let registry_url = if let Some(r) = registry_opt {
        r.trim_end_matches('/').to_string()
    } else {
        ctx.resolver.registry_url().trim_end_matches('/').to_string()
    };

    let token = credentials::get_token(&registry_url)
        .context(format!("No credentials found for registry {}. Please run 'kumo auth' first.", registry_url))?;

    let path_buf = std::path::Path::new(path);
    let pkg_json_path = path_buf.join("package.json");
    if !pkg_json_path.exists() {
        anyhow::bail!("No package.json found in directory {}", path);
    }
    let pkg_json_content = std::fs::read_to_string(&pkg_json_path)?;
    let pkg_json: serde_json::Value = serde_json::from_str(&pkg_json_content)?;

    let name = pkg_json.get("name").and_then(|n| n.as_str())
        .context("package.json is missing 'name' field")?;
    let version = pkg_json.get("version").and_then(|v| v.as_str())
        .context("package.json is missing 'version' field")?;

    println!("Packing package directory...");
    let tarball_bytes = kumo_core::tarball::pack_directory(path_buf)?;
    let tarball_len = tarball_bytes.len();

    let shasum = kumo_core::tarball::calculate_shasum(&tarball_bytes);
    let integrity = kumo_core::tarball::calculate_integrity(&tarball_bytes);

    let kumo_signature = if let Some(home) = dirs::home_dir() {
        let priv_key_path = home.join(".kumo").join("private_key.pem");
        if priv_key_path.exists() {
            if let Ok(private_key_pem) = std::fs::read_to_string(&priv_key_path) {
                keys::sign_payload(&private_key_pem, &integrity).ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let tarball_base64 = kumo_core::tarball::base64_encode(&tarball_bytes);
    let basename = name.split('/').last().unwrap_or(name);
    let tarball_filename = format!("{}-{}.tgz", basename, version);
    let tarball_url = format!("{}/{}/-/{}", registry_url, name, tarball_filename);

    let mut version_details = pkg_json.clone();
    let mut dist_json = serde_json::json!({
        "integrity": integrity,
        "shasum": shasum,
        "tarball": tarball_url
    });
    if let Some(sig) = kumo_signature {
        dist_json["kumoSignature"] = serde_json::Value::String(sig);
    }
    version_details["dist"] = dist_json;

    let publish_payload = serde_json::json!({
        "_id": name,
        "name": name,
        "description": pkg_json.get("description").unwrap_or(&serde_json::Value::Null),
        "dist-tags": {
            "latest": version
        },
        "versions": {
            version: version_details
        },
        "_attachments": {
            tarball_filename: {
                "content_type": "application/octet-stream",
                "data": tarball_base64,
                "length": tarball_len
            }
        }
    });

    println!("Publishing to {}...", registry_url);
    let url = format!("{}/{}", registry_url, name);

    let mut response = ctx.resolver.client()
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&publish_payload)
        .send()
        .await
        .context("Failed to communicate with registry")?;

    let mut status = response.status();
    let mut body_text = response.text().await.unwrap_or_default();

    if status.as_u16() == 401 || status.as_u16() == 403 {
        let is_otp_required = body_text.contains("OTP") || body_text.contains("one-time password") || body_text.contains("2fa");
        if is_otp_required {
            let otp: String = Input::new()
                .with_prompt("Enter NPM OTP code")
                .interact_text()?;

            response = ctx.resolver.client()
                .put(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("npm-otp", otp)
                .json(&publish_payload)
                .send()
                .await
                .context("Failed to communicate with registry on OTP retry")?;
            status = response.status();
            body_text = response.text().await.unwrap_or_default();
        }
    }

    if status.is_success() {
        println!("\x1b[32m✓\x1b[0m Published {}@{} successfully!", name, version);
        Ok(())
    } else {
        if let Ok(res_val) = serde_json::from_str::<serde_json::Value>(&body_text) {
            if let Some(reason) = res_val.get("reason").and_then(|r| r.as_str()) {
                anyhow::bail!("Publish failed: {}", reason);
            }
        }
        anyhow::bail!("Publish failed with status {}: {}", status, body_text);
    }
}
