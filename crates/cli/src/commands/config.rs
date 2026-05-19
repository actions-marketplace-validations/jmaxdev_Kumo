use anyhow::Result;
use crate::ConfigSubcommand;

pub async fn execute(subcommand: ConfigSubcommand) -> Result<()> {
    match subcommand {
        ConfigSubcommand::Init => {
            let config_path = std::env::current_dir()?.join("kumo.config.json");
            if config_path.exists() {
                anyhow::bail!("kumo.config.json already exists");
            }

            let policy = security::Policy::default();
            let json = serde_json::to_string_pretty(&policy)?;
            std::fs::write(config_path, json)?;
            println!("Created kumo.config.json with default security policies.");
        }
        ConfigSubcommand::Default { setting, value } => {
            let global_config = dirs::home_dir().unwrap().join(".kumo").join("kumo.config.json");
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
