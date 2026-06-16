use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub mod sandbox;
pub mod ast;
pub mod proxy;

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
    pub protected_packages: HashSet<String>,
    pub allowed_domains: HashSet<String>,
    pub registry: String,
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
            trust_policy_ignore_after: 10080,
            protected_packages: HashSet::new(),
            allowed_domains: vec![
                "github.com",
                "objects.githubusercontent.com",
                "registry.npmjs.org",
                "nodejs.org",
                "localhost",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            registry: "npm".to_string(),
        }
    }
}

pub struct SecurityEngine {
    pub policy: Policy,
    client: reqwest::Client,
    popular_packages: HashSet<String>,
}

impl SecurityEngine {
    pub fn new(policy: Policy) -> Self {
        let client = reqwest::Client::new();
        let popular_packages = Self::load_popular_packages_sync();
        Self {
            policy,
            client,
            popular_packages,
        }
    }

    fn get_popular_packages_path() -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|h| h.join(".kumo").join("top_packages.json"))
    }

    fn load_popular_packages_sync() -> HashSet<String> {
        if let Some(path) = Self::get_popular_packages_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(&content) {
                        return names.into_iter().collect();
                    }
                }
            }
        }
        HashSet::new()
    }

    fn needs_refresh() -> bool {
        if let Some(path) = Self::get_popular_packages_path() {
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        return elapsed.as_secs() > 7 * 24 * 60 * 60;
                    }
                }
            }
            return true;
        }
        true
    }

    pub async fn refresh_popular_packages(&mut self) -> Result<()> {
        if !Self::needs_refresh() {
            return Ok(());
        }

        let mut all_names: Vec<String> = Vec::new();

        let url = "https://data.jsdelivr.com/v1/stats/packages";
        if let Ok(resp) = self.client.get(url).send().await {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = data.as_array() {
                    for obj in arr {
                        if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                            if all_names.len() < 1000 {
                                all_names.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        if !all_names.is_empty() {
            if let Some(path) = Self::get_popular_packages_path() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let json = serde_json::to_string(&all_names)?;
                let _ = std::fs::write(&path, json);
            }
            self.popular_packages = all_names.into_iter().collect();
        }

        Ok(())
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


        if self.policy.trust_policy_ignore_after > 0 {
            if let Some(pub_at) = published_at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(pub_at) {
                    let now = chrono::Utc::now();
                    let age = now
                        .signed_duration_since(dt.with_timezone(&chrono::Utc))
                        .num_minutes();
                    if age > self.policy.trust_policy_ignore_after as i64 {
                        return true;
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
        if self.policy.trusted_packages.contains(name) {
            return Ok(true);
        }

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

    pub fn check_typosquatting(&self, name: &str, existing_deps: &HashSet<String>) -> Option<String> {

        let name_normalized = if name.starts_with('@') {
            name.split('/').nth(1).unwrap_or(name)
        } else {
            name
        };

        if self.policy.trusted_packages.contains(name)
            || self.policy.protected_packages.contains(name_normalized)
            || existing_deps.contains(name)
            || self.popular_packages.contains(name)
        {
            return None;
        }

        for pop in &self.popular_packages {
            let pop_normalized = if pop.starts_with('@') {
                pop.split('/').nth(1).unwrap_or(pop)
            } else {
                pop
            };
            if is_suspiciously_similar(name_normalized, pop_normalized) {
                return Some(pop.to_string());
            }
        }

        for pop in &self.policy.protected_packages {
            if is_suspiciously_similar(name_normalized, pop) {
                return Some(pop.to_string());
            }
        }

        for dep in existing_deps {
            let dep_normalized = if dep.starts_with('@') {
                dep.split('/').nth(1).unwrap_or(dep)
            } else {
                dep
            };
            if is_suspiciously_similar(name_normalized, dep_normalized) {
                return Some(dep.clone());
            }
        }

        None
    }

    pub fn validate_lockfile(&self, lockfile: &resolver::Lockfile) -> Result<()> {
        for (pkg_key, pkg_data) in &lockfile.packages {
            let (name, _version) = if pkg_key.contains('@') && !pkg_key.starts_with('@') {
                let parts: Vec<&str> = pkg_key.split('@').collect();
                (parts[0].to_string(), parts[1].to_string())
            } else if pkg_key.starts_with('@') {
                let parts: Vec<&str> = pkg_key.split('@').collect();
                if parts.len() >= 3 {
                    (format!("@{}", parts[1]), parts[2].to_string())
                } else {
                    (pkg_key.clone(), "".to_string())
                }
            } else {
                (pkg_key.clone(), "".to_string())
            };

            let tarball_url = &pkg_data.resolution.tarball;

            // 1. HTTPS Enforcement
            if !tarball_url.starts_with("https://") && !tarball_url.starts_with("git+https://") && !tarball_url.starts_with("git+ssh://") {
                anyhow::bail!(
                    "Lockfile injection detected! Package '{}' resolves to an insecure or unsupported URL scheme: {}",
                    name, tarball_url
                );
            }

            // 2. Host Validation
            if let Ok(parsed_url) = url::Url::parse(tarball_url) {
                if let Some(host) = parsed_url.host_str() {
                    let mut allowed = false;
                    for domain in &self.policy.allowed_domains {
                        if host == domain || host.ends_with(&format!(".{}", domain)) {
                            allowed = true;
                            break;
                        }
                    }
                    if !allowed {
                        anyhow::bail!(
                            "Lockfile injection detected! Package '{}' resolves to an untrusted host '{}'. URL: {}",
                            name, host, tarball_url
                        );
                    }
                }
            } else {
                anyhow::bail!("Lockfile injection detected! Invalid URL for package '{}': {}", name, tarball_url);
            }

            // 3. Package Name Alignment
            // The URL must contain the package name to prevent hijacking.
            // E.g., 'react' -> https://registry.npmjs.org/react/-/react-18.0.0.tgz
            let name_encoded = urlencoding::encode(&name);
            let name_encoded_scoped = name.replace('/', "%2f");
            if !tarball_url.contains(&name) && !tarball_url.contains(name_encoded.as_ref()) && !tarball_url.contains(&name_encoded_scoped) {
                anyhow::bail!(
                    "Lockfile injection detected! The resolved URL for '{}' does not appear to contain the package name. URL: {}",
                    name, tarball_url
                );
            }

            // 4. Integrity Validation
            let shasum = &pkg_data.resolution.shasum;
            if shasum.is_empty() {
                anyhow::bail!(
                    "Lockfile injection detected! Package '{}' is missing an integrity hash (shasum).",
                    name
                );
            }
            if shasum.len() < 40 {
                anyhow::bail!(
                    "Lockfile injection detected! Package '{}' has a suspiciously short integrity hash: {}",
                    name, shasum
                );
            }
        }

        Ok(())
    }
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let len_a = a_chars.len();
    let len_b = b_chars.len();

    if len_a == 0 { return len_b; }
    if len_b == 0 { return len_a; }

    let mut dp = vec![0; len_b + 1];
    for j in 0..=len_b {
        dp[j] = j;
    }

    for i in 1..=len_a {
        let mut prev = dp[0];
        dp[0] = i;
        for j in 1..=len_b {
            let temp = dp[j];
            if a_chars[i - 1] == b_chars[j - 1] {
                dp[j] = prev;
            } else {
                dp[j] = 1 + std::cmp::min(prev, std::cmp::min(dp[j], dp[j - 1]));
            }
            prev = temp;
        }
    }
    dp[len_b]
}

fn is_suspiciously_similar(a: &str, b: &str) -> bool {
    let len_a = a.chars().count();
    let len_b = b.chars().count();
    let min_len = std::cmp::min(len_a, len_b);
    let max_len = std::cmp::max(len_a, len_b);

    // Names ≤ 3 chars are too short for meaningful typosquatting detection
    if min_len <= 3 {
        return false;
    }

    // If length difference is too large, it's not typosquatting
    if max_len - min_len > 2 {
        return false;
    }

    // If one name is a prefix/suffix of the other with a separator, it's a variant
    // e.g. "lodash" and "lodash-es", "react" and "react-dom"
    if a.starts_with(b) || b.starts_with(a) {
        let longer = if len_a > len_b { a } else { b };
        let shorter = if len_a > len_b { b } else { a };
        let suffix = &longer[shorter.len()..];
        if suffix.starts_with('-') || suffix.starts_with('_') || suffix.starts_with('.') {
            return false;
        }
    }

    let dist = levenshtein_distance(a, b);
    if dist == 0 {
        return false;
    }

    // Tighter thresholds to reduce false positives
    if max_len <= 5 {
        dist == 1
    } else if max_len <= 10 {
        dist <= 1
    } else {
        dist <= 2
    }
}
