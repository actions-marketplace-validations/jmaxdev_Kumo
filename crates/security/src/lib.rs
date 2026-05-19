use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub mod sandbox;

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

    pub fn check_typosquatting(&self, name: &str, existing_deps: &HashSet<String>) -> Option<String> {
        // List of top 100 most popular npm packages to check against
        let popular_packages = [
            "react", "react-dom", "vue", "angular", "express", "lodash", "axios", "chalk", 
            "typescript", "vite", "esbuild", "tslib", "jest", "mocha", "dotenv", "webpack", 
            "rollup", "next", "nuxt", "gatsby", "commander", "minimist", "rimraf", "mkdirp", 
            "glob", "semver", "uuid", "moment", "inquirer", "debug", "bluebird", "async", 
            "request", "got", "node-fetch", "undici", "color", "colors", "prettier", "eslint", 
            "ts-node", "nodemon", "rxjs", "redux", "postcss", "tailwindcss", "autoprefixer", 
            "babel-core", "babel-loader", "clean-css", "css-loader", "style-loader", "file-loader", 
            "url-loader", "html-webpack-plugin", "mini-css-extract-plugin", "terser-webpack-plugin", 
            "source-map-support", "chokidar", "globby", "fast-glob", "jsdom", "cheerio", 
            "puppeteer", "playwright", "cypress", "tslint", "prettier-plugin-tailwindcss", 
            "cross-env", "shelljs", "execa", "ora", "cli-spinners", "yargs", "minimist", 
            "fs-extra", "graceful-fs", "promisify", "semver", "path-to-regexp", "body-parser", 
            "cors", "morgan", "helmet", "compression", "cookie-parser", "jsonwebtoken", 
            "bcrypt", "bcryptjs", "passport", "mongoose", "sequelize", "pg", "mysql2", 
            "redis", "nodemailer", "socket.io", "ws", "graphql", "apollo-server"
        ];

        // Normalize the name (remove scope if any)
        let name_normalized = if name.starts_with('@') {
            name.split('/').nth(1).unwrap_or(name)
        } else {
            name
        };

        // Don't flag exact matches
        if popular_packages.contains(&name_normalized) || existing_deps.contains(name) {
            return None;
        }

        // Check Levenshtein distance against popular packages
        for &pop in &popular_packages {
            if is_suspiciously_similar(name_normalized, pop) {
                return Some(pop.to_string());
            }
        }

        // Check Levenshtein distance against project's existing dependencies
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
    let dist = levenshtein_distance(a, b);
    if dist == 0 {
        return false;
    }

    let len_a = a.chars().count();
    let len_b = b.chars().count();
    let max_len = std::cmp::max(len_a, len_b);

    if max_len <= 4 {
        dist == 1
    } else if max_len <= 8 {
        dist <= 2
    } else {
        dist <= 3
    }
}
