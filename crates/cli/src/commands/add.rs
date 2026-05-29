use anyhow::Result;
use kumo_core::shield::ShieldManager;
use kumo_core::Store;
use resolver::Resolver;
use security::SecurityEngine;
use std::collections::HashMap;

#[derive(clap::Args)]
pub struct AddCommand {
    pub name: String,
    #[arg(short, long)]
    pub dev: bool,
    #[arg(short, long)]
    pub global: bool,
    #[arg(long)]
    pub log: bool,
}

#[async_trait::async_trait(?Send)]
impl super::Command for AddCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(&ctx.store, &ctx.resolver, &ctx.security, self.name.clone(), self.dev, self.global, self.log, ctx.config_path.clone()).await
    }
}

pub async fn execute(
    store: &Store,
    resolver: &Resolver,
    security: &SecurityEngine,
    name: String,
    dev: bool,
    global: bool,
    log: bool,
    config_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let (pkg_name, version_req) = crate::common::parse_package_arg(&name);

    if global {
        crate::commands::install::install_global(store, resolver, security, pkg_name, version_req).await?;
    } else {
        let config_path = config_path.ok_or_else(|| {
            anyhow::anyhow!("Neither kumo.json nor package.json found in current directory")
        })?;
        
        println!("Resolving package {}@{}...", pkg_name, version_req);
        let meta = resolver.resolve_package_fresh(&pkg_name, &version_req).await?;
        let resolved_version = meta.version.to_string();

        let version_to_save = if version_req == "latest" || version_req == "stable" || version_req == "*" || version_req.is_empty() {
            format!("^{}", resolved_version)
        } else {
            if version_req.chars().next().unwrap_or(' ').is_numeric() {
                format!("^{}", version_req)
            } else {
                version_req.clone()
            }
        };

        println!("Adding {}@{} to configuration...", pkg_name, version_to_save);
        let mut config_content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
        let section = if dev {
            "devDependencies"
        } else {
            "dependencies"
        };
        if let Some(obj) = config_content.as_object_mut() {
            obj.entry(section.to_string())
                .or_insert(serde_json::json!({}))
                .as_object_mut()
                .unwrap()
                .insert(pkg_name.clone(), serde_json::json!(version_to_save));
        }

        let json = serde_json::to_string_pretty(&config_content)?;
        let shield = ShieldManager::new();
        if shield.is_active() {
            let _ = shield.unshield_file(&config_path);
        }
        std::fs::write(&config_path, json)?;
        println!(
            "Updated {} with {}@{}",
            config_path.file_name().unwrap().to_string_lossy(),
            pkg_name,
            version_to_save
        );

        let mut deps = HashMap::new();
        deps.insert(pkg_name.clone(), version_to_save);
        crate::commands::install::resolve_and_install(
            store,
            resolver,
            security,
            deps,
            log,
            config_path,
        )
        .await?;
    }
    Ok(())
}
