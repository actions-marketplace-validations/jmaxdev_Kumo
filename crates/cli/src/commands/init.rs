use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Input};
use std::fs;

#[derive(clap::Args, Clone)]
pub struct InitCommand {
    #[arg(
        short = 'y',
        long = "yes",
        help = "Initialize package.json with default values without prompting"
    )]
    pub yes: bool,
}

#[async_trait::async_trait(?Send)]
impl super::Command for InitCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.yes).await
    }
}

pub async fn execute(yes: bool) -> Result<()> {
    let pkg_json_path = std::env::current_dir()?.join("package.json");
    if pkg_json_path.exists() {
        anyhow::bail!("package.json already exists");
    }

    let default_name = std::env::current_dir()?
        .file_name()
        .map(|name| {
            name.to_string_lossy()
                .to_string()
                .to_lowercase()
                .replace(' ', "-")
        })
        .unwrap_or_else(|| "project".to_string());

    let (name, version, description, main, test_script, keywords, author, license) = if yes {
        (
            default_name,
            "1.0.0".to_string(),
            "".to_string(),
            "index.js".to_string(),
            "echo \"Error: no test specified\" && exit 1".to_string(),
            vec![],
            "".to_string(),
            "ISC".to_string(),
        )
    } else {
        let theme = ColorfulTheme::default();

        let name: String = Input::with_theme(&theme)
            .with_prompt("package name")
            .default(default_name)
            .interact_text()?;

        let version: String = Input::with_theme(&theme)
            .with_prompt("version")
            .default("1.0.0".to_string())
            .interact_text()?;

        let description: String = Input::with_theme(&theme)
            .with_prompt("description")
            .default("".to_string())
            .allow_empty(true)
            .interact_text()?;

        let main: String = Input::with_theme(&theme)
            .with_prompt("entry point")
            .default("index.js".to_string())
            .interact_text()?;

        let test_script = "echo \"Error: no test specified\" && exit 1".to_string();
        let keywords: Vec<String> = vec![];

        let author: String = Input::with_theme(&theme)
            .with_prompt("author")
            .default("".to_string())
            .allow_empty(true)
            .interact_text()?;

        let license: String = Input::with_theme(&theme)
            .with_prompt("license")
            .default("ISC".to_string())
            .interact_text()?;

        (
            name,
            version,
            description,
            main,
            test_script,
            keywords,
            author,
            license,
        )
    };

    let pkg_json = serde_json::json!({
        "name": name,
        "version": version,
        "description": description,
        "main": main,
        "scripts": {
            "test": test_script
        },
        "keywords": keywords,
        "author": author,
        "license": license
    });

    let json_string = serde_json::to_string_pretty(&pkg_json)?;
    fs::write(&pkg_json_path, json_string)?;

    println!("Wrote to {}", pkg_json_path.display());
    Ok(())
}
