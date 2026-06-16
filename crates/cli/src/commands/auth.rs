use anyhow::{Context, Result};
use kumo_core::credentials;
use kumo_core::keys;
use std::time::Duration;
use tokio::time::sleep;

#[derive(clap::Args, Clone)]
pub struct AuthCommand {
    #[arg(long, help = "Custom registry URL to authenticate with")]
    pub registry: Option<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for AuthCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(ctx, self.registry.as_deref()).await
    }
}

fn open_browser(url: &str) -> std::io::Result<()> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()?;
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

pub async fn execute(
    ctx: &super::CommandContext,
    registry_opt: Option<&str>,
) -> Result<()> {
    // 1. Resolve registry URL
    let registry_url = if let Some(r) = registry_opt {
        r.trim_end_matches('/').to_string()
    } else {
        let resolved = ctx.resolver.registry_url().to_string();
        if resolved == "https://registry.npmjs.org" {
            "https://kumo.unsetsoft.com".to_string()
        } else {
            resolved
        }
    };

    println!("Starting authentication with registry: {}", registry_url);

    // 2. Get or generate key pair
    let (_priv_pem, pub_pem) = keys::get_or_create_keypair()?;

    // 3. Initiate web login session on the registry
    let url = format!("{}/-/v1/login", registry_url);
    let payload = serde_json::json!({
        "publicKey": pub_pem,
    });

    println!("Requesting authentication session...");
    let response = ctx.resolver.client()
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("Failed to communicate with registry")?;

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("Failed to start login session ({})", body_text);
    }

    let login_res: serde_json::Value = serde_json::from_str(&body_text)
        .context("Failed to parse login session response")?;

    let session_id = login_res.get("sessionId")
        .and_then(|s| s.as_str())
        .context("Missing sessionId in registry response")?;

    let login_url = login_res.get("loginUrl")
        .and_then(|u| u.as_str())
        .context("Missing loginUrl in registry response")?;

    // 4. Open loginUrl in browser
    println!("\n------------------------------------------------------------");
    println!("Please authenticate in your browser:");
    println!("{}", login_url);
    println!("------------------------------------------------------------\n");

    if let Err(_) = open_browser(login_url) {
        println!("Could not open browser automatically. Please copy the link above and open it manually.");
    } else {
        println!("Opening browser automatically...");
    }

    // 5. Poll registry for authentication status
    println!("Waiting for authorization...");
    let poll_url = format!("{}/-/v1/login/poll/{}", registry_url, session_id);

    loop {
        sleep(Duration::from_secs(2)).await;

        let poll_resp = match ctx.resolver.client().get(&poll_url).send().await {
            Ok(resp) => resp,
            Err(_) => continue, // Ignore network blips during polling
        };

        if !poll_resp.status().is_success() {
            continue;
        }

        let poll_body = poll_resp.text().await.unwrap_or_default();
        if let Ok(res_val) = serde_json::from_str::<serde_json::Value>(&poll_body) {
            let status_str = res_val.get("status").and_then(|s| s.as_str()).unwrap_or("pending");
            if status_str == "done" {
                if let Some(token) = res_val.get("token").and_then(|t| t.as_str()) {
                    let username = res_val.get("username").and_then(|u| u.as_str()).unwrap_or("unknown");
                    credentials::set_credential(&registry_url, username.to_string(), token.to_string())?;
                    println!("\x1b[32m✓\x1b[0m Successfully authenticated as \x1b[32m{}\x1b[0m!", username);
                    return Ok(());
                }
            } else if status_str == "failed" {
                anyhow::bail!("Authentication session failed or was denied by the user.");
            }
        }
    }
}
