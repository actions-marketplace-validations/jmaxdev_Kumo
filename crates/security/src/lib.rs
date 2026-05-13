use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub block_deprecated: bool,
    pub min_severity: String,
    pub blocked_packages: HashSet<String>,
    pub allowed_licenses: HashSet<String>,
    pub minimum_release_age: u64, // in minutes
    pub allow_postinstall: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            block_deprecated: true,
            min_severity: "high".to_string(),
            blocked_packages: HashSet::new(),
            allowed_licenses: vec!["MIT", "Apache-2.0", "ISC", "BSD-3-Clause"]
                .into_iter()
                .map(String::from)
                .collect(),
            minimum_release_age: 1440,
            allow_postinstall: false,
        }
    }
}

pub struct SecurityEngine {
    pub policy: Policy,
    client: reqwest::Client,
}

impl SecurityEngine {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            client: reqwest::Client::new(),
        }
    }

    /// Checks if a package version has known vulnerabilities via OSV.dev
    pub async fn check_vulnerabilities(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Vec<Vulnerability>> {
        let url = "https://api.osv.dev/v1/query";
        let body = serde_json::json!({
            "version": version,
            "package": {
                "name": name,
                "ecosystem": "npm"
            }
        });

        let response = self.client.post(url).json(&body).send().await?;
        if !response.status().is_success() {
            return Ok(vec![]); // Silent failure or handle error
        }

        let data: serde_json::Value = response.json().await?;
        let mut vulnerabilities = Vec::new();

        if let Some(vulns) = data.get("vulns").and_then(|v| v.as_array()) {
            for v in vulns {
                let id = v
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let summary = v
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or("No summary available")
                    .to_string();

                // OSV uses CVSS, we'll try to extract it or default to "Unknown"
                let severity = v
                    .get("database_specific")
                    .and_then(|d| d.get("severity"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("Medium")
                    .to_string();

                vulnerabilities.push(Vulnerability {
                    id,
                    summary,
                    severity,
                });
            }
        }

        Ok(vulnerabilities)
    }

    /// Validates if a package is "safe" to install based on current policies.
    pub async fn validate_package(
        &self,
        name: &str,
        version: &str,
        license: Option<&str>,
        is_deprecated: bool,
        published_at: Option<&str>,
        has_install_scripts: bool,
    ) -> Result<bool> {
        // 1. Blocked packages check
        if self.policy.blocked_packages.contains(name) {
            return Ok(false);
        }

        // 2. Deprecation check
        if self.policy.block_deprecated && is_deprecated {
            return Ok(false);
        }

        // 3. Minimum Release Age check (Mitigate 0-day malware)
        if self.policy.minimum_release_age > 0 {
            if let Some(pub_at) = published_at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(pub_at) {
                    let now = chrono::Utc::now();
                    let age = now
                        .signed_duration_since(dt.with_timezone(&chrono::Utc))
                        .num_minutes();
                    if age < self.policy.minimum_release_age as i64 {
                        return Ok(false);
                    }
                }
            }
        }

        // 4. Postinstall scripts check
        if !self.policy.allow_postinstall && has_install_scripts {
            // In a real app, we might want to warn or allow specific packages
            return Ok(false);
        }

        // 5. License check
        if let Some(lic) = license {
            if !self.policy.allowed_licenses.is_empty()
                && !self.policy.allowed_licenses.contains(lic)
            {
                return Ok(false);
            }
        }

        // 4. Vulnerability check
        let vulns = self.check_vulnerabilities(name, version).await?;
        for vuln in vulns {
            if self.is_severity_blocked(&vuln.severity) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn is_severity_blocked(&self, severity: &str) -> bool {
        let levels = ["low", "medium", "high", "critical"];
        let min_idx = levels
            .iter()
            .position(|&l| l == self.policy.min_severity)
            .unwrap_or(2);
        let curr_idx = levels
            .iter()
            .position(|&l| l == severity.to_lowercase())
            .unwrap_or(0);

        curr_idx >= min_idx
    }
}
