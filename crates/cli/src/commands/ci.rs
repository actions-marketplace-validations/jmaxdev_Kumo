use anyhow::Result;

#[derive(clap::Args)]
pub struct CiCommand {
    #[arg(long, default_value_t = true)]
    pub frozen: bool,

    #[arg(long, default_value_t = true)]
    pub audit: bool,

    #[arg(long, default_value_t = true)]
    pub ignore_scripts: bool,

    #[arg(long, default_value = "sarif", value_parser = ["sarif", "json", "text"])]
    pub format: String,

    #[arg(long)]
    pub audit_level: Option<String>,
}

#[async_trait::async_trait(?Send)]
impl super::Command for CiCommand {
    async fn run(&self, ctx: &super::CommandContext) -> anyhow::Result<()> {
        execute(ctx, self).await
    }
}

pub async fn execute(ctx: &super::CommandContext, args: &CiCommand) -> Result<()> {
    println!("kumo ci — secure CI pipeline");

    let config_path = ctx
        .config_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Neither kumo.json, package.json nor kumo.config.json found in current directory"))?;

    let lock_path = std::env::current_dir()?.join(kumo_core::config::KUMO_LOCK);

    if args.frozen {
        if !lock_path.exists() {
            anyhow::bail!(
                "CI frozen mode: kumo.lock not found. Run 'kumo install' locally and commit the lockfile."
            );
        }

        let config_content = std::fs::read_to_string(&config_path)?;
        let config_hash = blake3::hash(config_content.as_bytes()).to_string();

        let lockfile: resolver::Lockfile =
            serde_yml::from_str(&std::fs::read_to_string(&lock_path)?)?;

        if let Some(ref lf_hash) = lockfile.config_hash {
            if *lf_hash != config_hash {
                anyhow::bail!(
                    "CI frozen mode: kumo.lock is out of sync with {}. Run 'kumo install' locally and commit the updated lockfile.",
                    config_path.display()
                );
            }
        }
    }

    if args.audit {
        println!("Running security audit...");
        let scan_result = run_audit(&ctx.security, &lock_path, args.audit_level.as_deref()).await;

        match &scan_result {
            Ok(findings) if !findings.is_empty() => {
                match args.format.as_str() {
                    "sarif" => print_sarif(findings),
                    "json" => print_json(findings),
                    _ => print_text(findings),
                }
                anyhow::bail!(
                    "CI audit failed: {} vulnerabilities found above threshold",
                    findings.len()
                );
            }
            Ok(_) => println!("Audit passed: no vulnerabilities above threshold."),
            Err(e) => eprintln!("Audit warning: {}", e),
        }
    }

    println!("Installing dependencies (frozen={}, ignore-scripts={})...", args.frozen, args.ignore_scripts);

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

    if args.ignore_scripts {
        std::env::set_var("KUMO_IGNORE_SCRIPTS", "1");
    }

    super::install::resolve_and_install(
        &ctx.store,
        &ctx.resolver,
        &ctx.security,
        deps,
        false,
        config_path,
    )
    .await?;

    println!("kumo ci completed successfully.");
    Ok(())
}

#[derive(Clone)]
pub struct AuditFinding {
    pub package: String,
    pub version: String,
    pub vuln_id: String,
    pub severity: String,
    pub summary: String,
}

async fn run_audit(
    security: &security::SecurityEngine,
    lock_path: &std::path::Path,
    audit_level: Option<&str>,
) -> Result<Vec<AuditFinding>> {
    if !lock_path.exists() {
        return Ok(vec![]);
    }

    let lockfile: resolver::Lockfile =
        serde_yml::from_str(&std::fs::read_to_string(lock_path)?)?;
    let mut findings = Vec::new();
    let threshold = audit_level.unwrap_or("high");

    for (key, _pkg) in &lockfile.packages {
        let (name, version) = crate::common::parse_package_id(key);
        let vulns = security.check_vulnerabilities(&name, &version).await?;

        for v in vulns {
            if severity_meets_threshold(&v.severity, threshold) {
                findings.push(AuditFinding {
                    package: name.clone(),
                    version: version.clone(),
                    vuln_id: v.id,
                    severity: v.severity,
                    summary: v.summary,
                });
            }
        }
    }

    Ok(findings)
}

fn severity_meets_threshold(severity: &str, threshold: &str) -> bool {
    let levels = ["low", "moderate", "high", "critical"];
    let sev_idx = levels
        .iter()
        .position(|&l| l == severity.to_lowercase())
        .unwrap_or(0);
    let thr_idx = levels
        .iter()
        .position(|&l| l == threshold.to_lowercase())
        .unwrap_or(2);
    sev_idx >= thr_idx
}

fn print_sarif(findings: &[AuditFinding]) {
    let results: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "ruleId": f.vuln_id,
                "level": match f.severity.to_lowercase().as_str() {
                    "critical" | "high" => "error",
                    "moderate" => "warning",
                    _ => "note",
                },
                "message": {
                    "text": format!("{} ({}@{}): {}", f.vuln_id, f.package, f.version, f.summary)
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": "kumo.lock"
                        }
                    }
                }]
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "kumo",
                    "informationUri": "https://github.com/jmaxdev/Kumo",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            },
            "results": results
        }]
    });

    println!("{}", serde_json::to_string_pretty(&sarif).unwrap_or_default());
}

fn print_json(findings: &[AuditFinding]) {
    let items: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "package": f.package,
                "version": f.version,
                "id": f.vuln_id,
                "severity": f.severity,
                "summary": f.summary,
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "vulnerabilities": items }))
            .unwrap_or_default()
    );
}

fn print_text(findings: &[AuditFinding]) {
    for f in findings {
        eprintln!(
            "  [{}] {} ({}@{}): {}",
            f.severity, f.vuln_id, f.package, f.version, f.summary
        );
    }
}
