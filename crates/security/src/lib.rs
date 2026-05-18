use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub severity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrustLevel {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Low => write!(f, "Low"),
            TrustLevel::Medium => write!(f, "Medium"),
            TrustLevel::High => write!(f, "High"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub block_deprecated: bool,
    pub min_severity: String,
    pub blocked_packages: HashSet<String>,
    pub allowed_licenses: HashSet<String>,
    pub minimum_release_age: u64,
    pub allow_postinstall: bool,
    pub trusted_packages: HashSet<String>,
    pub trust_policy: String,
    pub trust_policy_exclude: HashSet<String>,
    pub trust_policy_ignore_after: u64,
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
            trusted_packages: HashSet::new(),
            trust_policy: "none".to_string(),
            trust_policy_exclude: HashSet::new(),
            trust_policy_ignore_after: 10080, // 7 days in minutes
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

    pub fn get_trust_level(&self, has_signatures: bool, has_attestations: bool) -> TrustLevel {
        if has_attestations {
            TrustLevel::High
        } else if has_signatures {
            TrustLevel::Medium
        } else {
            TrustLevel::Low
        }
    }

    pub fn validate_trust_downgrade(
        &self,
        name: &str,
        new_level: TrustLevel,
        old_level: TrustLevel,
        published_at: Option<&str>,
    ) -> bool {
        if self.policy.trust_policy != "no-downgrade" {
            return true;
        }

        if self.policy.trust_policy_exclude.contains(name) {
            return true;
        }

        if new_level >= old_level {
            return true;
        }

        // Check trust_policy_ignore_after
        if self.policy.trust_policy_ignore_after > 0 {
            if let Some(pub_at) = published_at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(pub_at) {
                    let now = chrono::Utc::now();
                    let age = now
                        .signed_duration_since(dt.with_timezone(&chrono::Utc))
                        .num_minutes();
                    if age > self.policy.trust_policy_ignore_after as i64 {
                        return true; // Ignored because package is too old
                    }
                }
            }
        }

        false
    }

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
            return Ok(vec![]);
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

    pub async fn validate_package(
        &self,
        name: &str,
        version: &str,
        license: Option<&str>,
        is_deprecated: bool,
        published_at: Option<&str>,
        has_install_scripts: bool,
    ) -> Result<bool> {
        if self.policy.blocked_packages.contains(name) {
            return Ok(false);
        }

        if self.policy.block_deprecated && is_deprecated {
            return Ok(false);
        }

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

        if !self.policy.allow_postinstall
            && has_install_scripts
            && !self.policy.trusted_packages.contains(name)
        {
            return Ok(false);
        }

        if let Some(lic) = license {
            if !self.policy.allowed_licenses.is_empty()
                && !self.policy.allowed_licenses.contains(lic)
            {
                return Ok(false);
            }
        }

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
