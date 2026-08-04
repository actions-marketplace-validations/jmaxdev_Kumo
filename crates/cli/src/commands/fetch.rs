use anyhow::Result;
use kumo_core::Store;
use resolver::Resolver;

#[derive(clap::Args)]
pub struct FetchCommand;

#[async_trait::async_trait(?Send)]
impl super::Command for FetchCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(&ctx.store, &ctx.resolver, ctx.config_path.clone()).await
    }
}

pub async fn execute(
    store: &Store,
    resolver: &Resolver,
    config_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let config_path = config_path.ok_or_else(|| {
        anyhow::anyhow!("Neither kumo.json, package.json nor kumo.config.json found in current directory")
    })?;

    println!("Reading configuration...");
    let config_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;
    let mut deps = std::collections::HashMap::new();

    if let Some(d) = config_content.get("dependencies").and_then(|v| v.as_object()) {
        for (k, v) in d {
            deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
        }
    }
    if let Some(d) = config_content.get("devDependencies").and_then(|v| v.as_object()) {
        for (k, v) in d {
            deps.insert(k.clone(), v.as_str().unwrap_or("latest").to_string());
        }
    }

    let lock_path = std::env::current_dir()?.join(kumo_core::config::KUMO_LOCK);
    let lockfile = if lock_path.exists() {
        serde_yml::from_str(&std::fs::read_to_string(&lock_path)?)?
    } else {
        println!("Resolving dependency tree...");
        let mut lf = resolver.resolve_tree(&deps).await?;
        lf.config_hash = Some(blake3::hash(std::fs::read_to_string(&config_path)?.as_bytes()).to_string());
        let yaml = serde_yml::to_string(&lf)?;
        std::fs::write(&lock_path, yaml)?;
        lf
    };

    let total = lockfile.packages.len();
    println!("Fetching {} packages to store...", total);

    let pb = indicatif::ProgressBar::new(total as u64);
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    let mut cached = 0u64;
    let mut downloaded = 0u64;

    for (key, pkg) in &lockfile.packages {
        let (name, _version) = crate::common::parse_package_id(key);
        pb.set_message(name.clone());

        if let Ok(Some(_)) = store.load_index(key).await {
            cached += 1;
            pb.inc(1);
            continue;
        }

        let response = resolver.client().get(&pkg.resolution.tarball).send().await?;
        let bytes = response.bytes().await?;

        kumo_core::tarball::verify_shasum(&bytes, &pkg.resolution.shasum)?;

        let file_map = kumo_core::tarball::extract_and_store(store, &bytes).await?;
        store.save_index(key, &file_map).await?;

        downloaded += 1;
        pb.inc(1);
    }

    pb.finish_and_clear();
    println!(
        "Fetch complete: {} cached, {} downloaded. Store ready for offline install.",
        cached, downloaded
    );

    Ok(())
}
