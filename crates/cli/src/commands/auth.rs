use anyhow::{Context, Result};
use dialoguer::{Input, Password};
use kumo_core::credentials;

#[derive(clap::Args, Clone)]
pub struct AuthCommand {
    #[arg(long, help = "Custom registry URL to authenticate with")]
    pub registry: Option<String>,

    #[arg(long, help = "NPM authentication token")]
    pub token: Option<String>,

    #[arg(long, help = "NPM username")]
    pub username: Option<String>,

    #[arg(long, help = "NPM password")]
    pub password: Option<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for AuthCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(
            ctx,
            self.registry.as_deref(),
            self.token.as_deref(),
            self.username.as_deref(),
            self.password.as_deref(),
        )
        .await
    }
}

pub async fn execute(
    ctx: &super::CommandContext,
    registry_opt: Option<&str>,
    token_opt: Option<&str>,
    username_opt: Option<&str>,
    password_opt: Option<&str>,
) -> Result<()> {
    let registry_url = if let Some(r) = registry_opt {
        r.trim_end_matches('/').to_string()
    } else {
        ctx.resolver.registry_url().trim_end_matches('/').to_string()
    };

    println!("Authenticating with registry: {}", registry_url);

    if let Some(token) = token_opt {
        let username = username_opt.unwrap_or("user");
        credentials::set_credential(&registry_url, username.to_string(), token.to_string())?;
        println!(
            "\x1b[32m✓\x1b[0m Successfully stored NPM authentication token for \x1b[32m{}\x1b[0m!",
            registry_url
        );
        return Ok(());
    }

    let username = match username_opt {
        Some(u) => u.to_string(),
        None => Input::<String>::new()
            .with_prompt("Username")
            .interact_text()?,
    };

    let password = match password_opt {
        Some(p) => p.to_string(),
        None => Password::new()
            .with_prompt("Password")
            .interact()?,
    };

    let url = format!("{}/-/user/org.couchdb.user:{}", registry_url, username);
    let payload = serde_json::json!({
        "_id": format!("org.couchdb.user:{}", username),
        "name": username,
        "password": password,
        "type": "user",
        "roles": []
    });

    let response = ctx.resolver.client()
        .put(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .context("Failed to communicate with registry")?;

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("NPM authentication failed ({}): {}", status, body_text);
    }

    let login_res: serde_json::Value = serde_json::from_str(&body_text)
        .unwrap_or_else(|_| serde_json::json!({}));

    let token = login_res
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            kumo_core::tarball::base64_encode(format!("{}:{}", username, password).as_bytes())
        });

    credentials::set_credential(&registry_url, username.clone(), token)?;
    println!(
        "\x1b[32m✓\x1b[0m Successfully authenticated as \x1b[32m{}\x1b[0m!",
        username
    );

    Ok(())
}
