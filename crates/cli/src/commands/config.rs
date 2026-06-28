use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Clone)]
pub enum ConfigSubcommand {
    Init,
    Default {
        setting: String,
        value: String,
    },
}

#[derive(clap::Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub subcommand: ConfigSubcommand,
}

#[async_trait::async_trait(?Send)]
impl super::Command for ConfigCommand {
    async fn run(&self, _ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(self.subcommand.clone()).await
    }
}

pub async fn execute(subcommand: ConfigSubcommand) -> Result<()> {
    match subcommand {
        ConfigSubcommand::Init => {
            let config_path = std::env::current_dir()?.join(kumo_core::config::KUMO_CONFIG_JSON);
            if config_path.exists() {
                anyhow::bail!("{} already exists", kumo_core::config::KUMO_CONFIG_JSON);
            }

            let policy = security::Policy::default();
            let json = serde_json::to_string_pretty(&policy)?;
            std::fs::write(&config_path, json)?;
            println!("Created {} with default security policies.", kumo_core::config::KUMO_CONFIG_JSON);
        }
        ConfigSubcommand::Default { setting, value } => {
            let global_config = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?.join(kumo_core::config::KUMO_DIR_NAME).join(kumo_core::config::KUMO_CONFIG_JSON);
            let mut config_json = if global_config.exists() {
                serde_json::from_str(&std::fs::read_to_string(&global_config)?).unwrap_or_else(|_| serde_json::to_value(security::Policy::default()).unwrap())
            } else {
                serde_json::to_value(security::Policy::default())?
            };

            let parsed_value = if value == "true" {
                serde_json::json!(true)
            } else if value == "false" {
                serde_json::json!(false)
            } else if let Ok(n) = value.parse::<i64>() {
                serde_json::json!(n)
            } else {
                serde_json::json!(value)
            };

            config_json.as_object_mut().unwrap().insert(setting.clone(), parsed_value);
            
            if let Some(parent) = global_config.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            
            std::fs::write(&global_config, serde_json::to_string_pretty(&config_json)?)?;
            println!("Global configuration updated: {} = {}", setting, value);
        }
    }
    Ok(())
}
