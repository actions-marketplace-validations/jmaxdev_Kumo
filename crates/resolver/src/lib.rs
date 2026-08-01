use anyhow::{anyhow, Context, Result};
use semver::{Version, VersionReq};
use std::collections::HashMap;

mod types;
pub use types::*;
mod cache;

#[derive(Clone)]
pub struct Resolver {
    client: reqwest::Client,
    registry_url: String,
}

fn normalize_version_str(v: &str) -> String {
    let clean = v.trim().trim_start_matches('v').trim_start_matches('=');
    if clean.is_empty() || clean == "*" || clean.eq_ignore_ascii_case("x") {
        return "*".to_string();
    }

    let (ver_part, extra) = match clean.find('-') {
        Some(idx) => (&clean[..idx], &clean[idx..]),
        None => (clean, ""),
    };

    let parts: Vec<&str> = ver_part.split('.').collect();
    match parts.len() {
        1 => {
            if parts[0] == "*" || parts[0].eq_ignore_ascii_case("x") {
                "*".to_string()
            } else if parts[0].chars().all(|c| c.is_ascii_digit()) {
                format!("{}.0.0{}", parts[0], extra)
            } else {
                clean.to_string()
            }
        }
        2 => {
            let p0 = if parts[0] == "*" || parts[0].eq_ignore_ascii_case("x") { "*" } else { parts[0] };
            let p1 = if parts[1] == "*" || parts[1].eq_ignore_ascii_case("x") { "*" } else { parts[1] };
            if p1 == "*" {
                if p0 == "*" { "*".to_string() } else { format!("{}.*", p0) }
            } else if p0.chars().all(|c| c.is_ascii_digit()) && p1.chars().all(|c| c.is_ascii_digit()) {
                format!("{}.{}.0{}", p0, p1, extra)
            } else {
                clean.to_string()
            }
        }
        _ => {
            let mut normalized_parts = Vec::new();
            for p in parts {
                if p == "*" || p.eq_ignore_ascii_case("x") {
                    normalized_parts.push("*");
                } else {
                    normalized_parts.push(p);
                }
            }
            format!("{}{}", normalized_parts.join("."), extra)
        }
    }
}

fn normalize_hyphen_range(left: &str, right: &str) -> String {
    let left_clean = left.trim().trim_start_matches('v').trim_start_matches('=');
    let right_clean = right.trim().trim_start_matches('v').trim_start_matches('=');

    let (l_ver, l_extra) = match left_clean.find('-') {
        Some(idx) => (&left_clean[..idx], &left_clean[idx..]),
        None => (left_clean, ""),
    };
    let l_parts: Vec<&str> = l_ver.split('.').collect();
    let left_req = match l_parts.len() {
        1 => {
            if l_parts[0].chars().all(|c| c.is_ascii_digit()) {
                format!(">={}.0.0{}", l_parts[0], l_extra)
            } else {
                format!(">={}{}", l_ver, l_extra)
            }
        }
        2 => {
            if l_parts[0].chars().all(|c| c.is_ascii_digit()) && l_parts[1].chars().all(|c| c.is_ascii_digit()) {
                format!(">={}.{}.0{}", l_parts[0], l_parts[1], l_extra)
            } else {
                format!(">={}{}", l_ver, l_extra)
            }
        }
        _ => format!(">={}{}", l_ver, l_extra),
    };

    let (r_ver, _r_extra) = match right_clean.find('-') {
        Some(idx) => (&right_clean[..idx], &right_clean[idx..]),
        None => (right_clean, ""),
    };
    let r_parts: Vec<&str> = r_ver.split('.').collect();
    let right_req = match r_parts.len() {
        1 => {
            if let Ok(n) = r_parts[0].parse::<u64>() {
                format!("<{}.0.0", n + 1)
            } else {
                format!("<={}.0.0", r_parts[0])
            }
        }
        2 => {
            if let (Ok(m), Ok(n)) = (r_parts[0].parse::<u64>(), r_parts[1].parse::<u64>()) {
                format!("<{}.{}.0", m, n + 1)
            } else {
                format!("<={}.{}.0", r_parts[0], r_parts[1])
            }
        }
        _ => format!("<={}", right_clean),
    };

    format!("{}, {}", left_req, right_req)
}

fn normalize_npm_semver_clause(r: &str) -> String {
    let r = r.trim();
    if r.contains(" - ") {
        let parts: Vec<&str> = r.splitn(2, " - ").collect();
        if parts.len() == 2 {
            return normalize_hyphen_range(parts[0], parts[1]);
        }
    }

    let preprocessed = r
        .replace(">= ", ">=")
        .replace("<= ", "<=")
        .replace("> ", ">")
        .replace("< ", "<")
        .replace("^ ", "^")
        .replace("~ ", "~")
        .replace("= ", "=");

    let tokens = preprocessed.split_whitespace();
    let mut normalized_tokens = Vec::new();

    for token in tokens {
        let (op, v_raw) = if let Some(stripped) = token.strip_prefix(">=") {
            (">=", stripped)
        } else if let Some(stripped) = token.strip_prefix("<=") {
            ("<=", stripped)
        } else if let Some(stripped) = token.strip_prefix('>') {
            (">", stripped)
        } else if let Some(stripped) = token.strip_prefix('<') {
            ("<", stripped)
        } else if let Some(stripped) = token.strip_prefix('^') {
            ("^", stripped)
        } else if let Some(stripped) = token.strip_prefix('~') {
            ("~", stripped)
        } else if let Some(stripped) = token.strip_prefix('=') {
            ("=", stripped)
        } else {
            ("", token)
        };

        let v_no_v = if v_raw.starts_with('v') && v_raw.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
            &v_raw[1..]
        } else {
            v_raw
        };

        if op.is_empty() {
            if v_no_v == "*" || v_no_v.eq_ignore_ascii_case("x") || v_no_v.is_empty() {
                normalized_tokens.push("*".to_string());
            } else if v_no_v.ends_with(".x") || v_no_v.ends_with(".X") {
                let stem = &v_no_v[..v_no_v.len() - 2];
                normalized_tokens.push(format!("{}.*", stem));
            } else {
                normalized_tokens.push(v_no_v.to_string());
            }
        } else {
            let (ver_part, _extra) = match v_no_v.find('-') {
                Some(idx) => (&v_no_v[..idx], &v_no_v[idx..]),
                None => (v_no_v, ""),
            };
            let parts: Vec<&str> = ver_part.split('.').collect();

            if op == "<=" && parts.len() == 1 {
                if let Ok(n) = parts[0].parse::<u64>() {
                    normalized_tokens.push(format!("<{}.0.0", n + 1));
                    continue;
                }
            } else if op == "<=" && parts.len() == 2 {
                if let (Ok(m), Ok(n)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                    normalized_tokens.push(format!("<{}.{}.0", m, n + 1));
                    continue;
                }
            }

            let norm_v = normalize_version_str(v_no_v);
            normalized_tokens.push(format!("{}{}", op, norm_v));
        }
    }

    normalized_tokens.join(",")
}

pub fn parse_version_reqs(range: &str) -> Result<Vec<VersionReq>> {
    let range_clean = range.trim();
    if range_clean.is_empty() || range_clean == "*" || range_clean.eq_ignore_ascii_case("x") || range_clean == "latest" {
        return Ok(vec![VersionReq::STAR]);
    }

    range_clean
        .split("||")
        .map(|r| {
            let normalized = normalize_npm_semver_clause(r);
            VersionReq::parse(&normalized)
                .map_err(|e| anyhow!("Invalid semver range '{}' (normalized: '{}'): {}", range, normalized, e))
        })
        .collect::<Result<Vec<_>, _>>()
}

impl Resolver {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .pool_max_idle_per_host(kumo_core::config::RESOLVER_POOL_MAX_IDLE)
            .pool_idle_timeout(std::time::Duration::from_secs(kumo_core::config::RESOLVER_IDLE_TIMEOUT_SECS))
            .tcp_nodelay(true)
            .user_agent(kumo_core::config::DEFAULT_USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut registry_raw = kumo_core::config::DEFAULT_REGISTRY.to_string();

        if let Ok(env_reg) = std::env::var(kumo_core::config::ENV_VAR_KUMO_REGISTRY) {
            if !env_reg.trim().is_empty() {
                registry_raw = env_reg.trim().to_string();
            }
        } else {
            let mut found = false;
            if let Ok(curr_dir) = std::env::current_dir() {
                let local_path = curr_dir.join(kumo_core::config::KUMO_CONFIG_JSON);
                if let Ok(content) = std::fs::read_to_string(&local_path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(r) = val.get("registry").and_then(|v| v.as_str()) {
                            registry_raw = r.to_string();
                            found = true;
                        }
                    }
                }
            }
            if !found {
                if let Some(home) = dirs::home_dir() {
                    let global_path = home.join(kumo_core::config::KUMO_DIR_NAME).join(kumo_core::config::KUMO_CONFIG_JSON);
                    if let Ok(content) = std::fs::read_to_string(&global_path) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(r) = val.get("registry").and_then(|v| v.as_str()) {
                                registry_raw = r.to_string();
                            }
                        }
                    }
                }
            }
        }

        let registry_url = match registry_raw.as_str() {
            "npm" => kumo_core::config::DEFAULT_REGISTRY_NPM_URL.to_string(),
            "kumo" => kumo_core::config::DEFAULT_REGISTRY_KUMO_URL.to_string(),
            other if other.starts_with("http://") || other.starts_with("https://") => {
                other.trim_end_matches('/').to_string()
            }
            _ => kumo_core::config::DEFAULT_REGISTRY_NPM_URL.to_string(),
        };

        Self {
            client,
            registry_url,
        }
    }

    pub fn registry_url(&self) -> &str {
        &self.registry_url
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn resolve_package(self, name: String, range: String) -> Result<PackageMetadata> {
        self.resolve_package_internal(&name, &range).await
    }

    pub async fn resolve_package_fresh(&self, name: &str, range: &str) -> Result<PackageMetadata> {
        let cache_path = cache::get_metadata_cache_path(name);
        let _ = std::fs::remove_file(&cache_path);

        self.resolve_package_internal(name, range).await
    }

    pub async fn get_latest_version(&self, name: &str) -> Result<String> {
        let cache_path = cache::get_metadata_cache_path(name);
        let _ = std::fs::remove_file(&cache_path);

        let response = self.fetch_and_cache_metadata(name, &cache_path).await?;
        response
            .dist_tags
            .get("latest")
            .cloned()
            .ok_or_else(|| anyhow!("No latest tag found for {}", name))
    }

    pub async fn get_available_versions(&self, name: &str) -> Result<Vec<Version>> {
        let cache_path = cache::get_metadata_cache_path(name);

        let response: RegistryResponse = if cache_path.exists() {
            let content = std::fs::read_to_string(&cache_path)?;
            if let Ok(res) = serde_json::from_str::<RegistryResponse>(&content) {
                if res.versions.is_empty() {
                    self.fetch_and_cache_metadata(name, &cache_path).await?
                } else {
                    res
                }
            } else {
                self.fetch_and_cache_metadata(name, &cache_path).await?
            }
        } else {
            self.fetch_and_cache_metadata(name, &cache_path).await?
        };

        let mut versions: Vec<Version> = response
            .versions
            .keys()
            .filter_map(|v| Version::parse(v).ok())
            .collect();
        versions.sort();
        Ok(versions)
    }

    async fn resolve_package_internal(&self, name: &str, range: &str) -> Result<PackageMetadata> {
        let cache_path = cache::get_metadata_cache_path(name);

        let mut from_cache = false;
        let mut response: RegistryResponse = if cache_path.exists() {
            let content = std::fs::read_to_string(&cache_path)?;
            if let Ok(res) = serde_json::from_str::<RegistryResponse>(&content) {
                if res.versions.is_empty() {
                    self.fetch_and_cache_metadata(name, &cache_path).await?
                } else {
                    from_cache = true;
                    res
                }
            } else {
                self.fetch_and_cache_metadata(name, &cache_path).await?
            }
        } else {
            self.fetch_and_cache_metadata(name, &cache_path).await?
        };

        let mut attempts = 0;
        let version_str = loop {
            attempts += 1;

            let resolved = if response.dist_tags.contains_key(range) {
                Some(response.dist_tags.get(range).unwrap().to_string())
            } else if range == "latest" || range == "*" || range == "" {
                response.dist_tags.get("latest").cloned()
            } else {
                match parse_version_reqs(range) {
                    Ok(reqs) => {
                        let mut versions: Vec<Version> = response
                            .versions
                            .keys()
                            .filter_map(|v| Version::parse(v).ok())
                            .filter(|v| reqs.iter().any(|req| req.matches(v)))
                            .collect();
                        versions.sort();
                        versions.last().map(|v| v.to_string())
                    }
                    Err(_) => None,
                }
            };

            if let Some(v_str) = resolved {
                if response.versions.contains_key(&v_str) {
                    break v_str;
                }
            }

            if from_cache && attempts < 2 {
                response = self.fetch_and_cache_metadata(name, &cache_path).await?;
                from_cache = false;
            } else {
                let reqs_res = parse_version_reqs(range);

                if reqs_res.is_err() {
                    anyhow::bail!("Invalid semver range: {}", range);
                }

                if response.dist_tags.contains_key(range) {
                    let v_str = response.dist_tags.get(range).unwrap();
                    anyhow::bail!(
                        "Version data for {} not found (resolved from tag {})",
                        v_str,
                        range
                    );
                } else if range == "latest" || range == "*" || range == "" {
                    let v_str = response
                        .dist_tags
                        .get("latest")
                        .ok_or_else(|| anyhow!("No latest tag found for {}", name))?;
                    anyhow::bail!(
                        "Version data for {} not found (resolved from latest tag)",
                        v_str
                    );
                } else {
                    anyhow::bail!("No version matching {} found for {}", range, name);
                }
            }
        };

        let version_data = response
            .versions
            .get(&version_str)
            .ok_or_else(|| anyhow!("Version data for {} not found", version_str))?;

        let published_at = response
            .time
            .as_ref()
            .and_then(|t| t.get(&version_str))
            .cloned();

        let license = version_data.license.as_ref().and_then(|l| {
            if let Some(s) = l.as_str() {
                Some(s.to_string())
            } else {
                l.get("type")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            }
        });

        let deprecated = version_data.deprecated.as_ref().and_then(|d| {
            if let Some(s) = d.as_str() {
                Some(s.to_string())
            } else {
                None
            }
        });

        Ok(PackageMetadata {
            name: version_data.name.clone(),
            version: Version::parse(&version_data.version)?,
            dependencies: version_data.dependencies.clone(),
            dist: version_data.dist.clone(),
            license,
            deprecated,
            published_at,
            bin: version_data.bin.clone(),
            scripts: version_data.scripts.clone(),
            optional_dependencies: version_data.optional_dependencies.clone(),
            os: version_data.os.clone(),
            cpu: version_data.cpu.clone(),
        })
    }

    fn is_compatible(os_list: &Option<Vec<String>>, cpu_list: &Option<Vec<String>>) -> bool {
        let current_os = match std::env::consts::OS {
            "windows" => "win32",
            "macos" => "darwin",
            os => os,
        };
        let current_arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "ia32",
            "aarch64" => "arm64",
            "powerpc" => "ppc",
            "powerpc64" => "ppc64",
            arch => arch,
        };

        if let Some(os) = os_list {
            if !os.is_empty() {
                let mut match_found = false;
                let mut has_negation = false;
                for o in os {
                    if o.starts_with('!') {
                        has_negation = true;
                        if &o[1..] == current_os {
                            return false;
                        }
                    } else if o == current_os {
                        match_found = true;
                    }
                }
                if !match_found && !has_negation {
                    return false;
                }
            }
        }

        if let Some(cpu) = cpu_list {
            if !cpu.is_empty() {
                let mut match_found = false;
                let mut has_negation = false;
                for c in cpu {
                    if c.starts_with('!') {
                        has_negation = true;
                        if &c[1..] == current_arch {
                            return false;
                        }
                    } else if c == current_arch {
                        match_found = true;
                    }
                }
                if !match_found && !has_negation {
                    return false;
                }
            }
        }

        true
    }

    pub async fn resolve_tree(&self, root_deps: &HashMap<String, String>) -> Result<Lockfile> {
        use futures::future::BoxFuture;
        use futures::stream::{FuturesUnordered, StreamExt};
        use std::sync::{Arc, Mutex};

        let packages = Arc::new(Mutex::new(HashMap::new()));
        let resolved_root_deps = Arc::new(Mutex::new(HashMap::new()));
        let queue = Arc::new(Mutex::new(
            root_deps
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
        ));

        let mut cache_empty = true;
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".kumo")
            .join("cache")
            .join("metadata");
        if cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(cache_dir) {
                if entries.count() > 10 {
                    cache_empty = false;
                }
            }
        }

        if cache_empty {
            println!("Note: Metadata cache is empty or small. Resolution may take longer as we fetch from registry.");
        }

        let resolver = Arc::new(self.clone());
        let mut active_resolutions: FuturesUnordered<
            BoxFuture<'static, (String, Result<PackageMetadata>)>,
        > = FuturesUnordered::new();

        let mut initial_queue = queue.lock().unwrap();
        while let Some(item) = initial_queue.pop() {
            let r = (*resolver).clone();
            active_resolutions.push(Box::pin(async move {
                let res = r.resolve_package(item.0.clone(), item.1.clone()).await;
                (item.0, res)
            }));
        }
        drop(initial_queue);

        while let Some((req_name, metadata_res)) = active_resolutions.next().await {
            let metadata = metadata_res?;

            if !Self::is_compatible(&metadata.os, &metadata.cpu) {
                continue;
            }

            let key = format!("{}@{}", metadata.name, metadata.version);

            let mut pkgs = packages.lock().unwrap();
            if !pkgs.contains_key(&key) {
                let mut all_deps = metadata.dependencies.clone().unwrap_or_default();
                if let Some(opt_deps) = &metadata.optional_dependencies {
                    for (k, v) in opt_deps {
                        all_deps.insert(k.clone(), v.clone());
                    }
                }

                pkgs.insert(
                    key.clone(),
                    LockedPackage {
                        resolution: metadata.dist.clone(),
                        dependencies: metadata.dependencies.clone(),
                        bin: metadata.bin.clone(),
                        scripts: metadata.scripts.clone(),
                        optional_dependencies: metadata.optional_dependencies.clone(),
                        published_at: metadata.published_at.clone(),
                    },
                );
                drop(pkgs);

                for (d_name, d_range) in all_deps {
                    let r = (*resolver).clone();
                    active_resolutions.push(Box::pin(async move {
                        let res = r.resolve_package(d_name.clone(), d_range.clone()).await;
                        (d_name, res)
                    }));
                }
            } else {
                drop(pkgs);
            }

            if root_deps.contains_key(&req_name) {
                resolved_root_deps
                    .lock()
                    .unwrap()
                    .insert(req_name, metadata.version.to_string());
            }
        }

        let final_packages = Arc::try_unwrap(packages).unwrap().into_inner().unwrap();
        let final_root_deps = Arc::try_unwrap(resolved_root_deps)
            .unwrap()
            .into_inner()
            .unwrap();

        Ok(Lockfile {
            lockfile_version: "1.0".to_string(),
            config_hash: None,
            dependencies: final_root_deps,
            packages: final_packages,
        })
    }

    async fn fetch_and_cache_metadata(
        &self,
        name: &str,
        cache_path: &std::path::Path,
    ) -> Result<RegistryResponse> {
        let url = format!("{}/{}", self.registry_url, name);
        let mut last_err = None;

        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt)).await;
            }

            match self.client.get(&url).send().await {
                Ok(res) => {
                    let metadata: RegistryResponse = res
                        .json()
                        .await
                        .with_context(|| format!("Failed to parse metadata for {}", name))?;
                    let json = serde_json::to_string(&metadata)?;
                    let shield = kumo_core::shield::ShieldManager::new();
                    if shield.is_active() {
                        let _ = shield.unshield_file(cache_path);
                    }
                    let _ = std::fs::write(cache_path, json);
                    if shield.is_active() {
                        let _ = shield.shield_file(cache_path);
                    }
                    return Ok(metadata);
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(anyhow!(
            "Failed to fetch metadata for {} after 3 attempts: {:?}",
            name,
            last_err
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_reqs() {
        let reqs = parse_version_reqs("^0.34.5").unwrap();
        let v = Version::parse("0.34.5").unwrap();
        assert!(reqs.iter().any(|r| r.matches(&v)));

        let reqs_v = parse_version_reqs("v0.34.5").unwrap();
        assert!(reqs_v.iter().any(|r| r.matches(&v)));

        let reqs_space = parse_version_reqs("^ 0.34.5").unwrap();
        assert!(reqs_space.iter().any(|r| r.matches(&v)));

        // Hyphen ranges (e.g. 1 - 3)
        let reqs_hyphen = parse_version_reqs("1 - 3").unwrap();
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.5.0").unwrap();
        let v3 = Version::parse("3.9.9").unwrap();
        let v4 = Version::parse("4.0.0").unwrap();
        assert!(reqs_hyphen.iter().any(|r| r.matches(&v1)));
        assert!(reqs_hyphen.iter().any(|r| r.matches(&v2)));
        assert!(reqs_hyphen.iter().any(|r| r.matches(&v3)));
        assert!(!reqs_hyphen.iter().any(|r| r.matches(&v4)));

        let reqs_hyphen_full = parse_version_reqs("1.0.0 - 2.0.0").unwrap();
        assert!(reqs_hyphen_full.iter().any(|r| r.matches(&v1)));
        assert!(reqs_hyphen_full.iter().any(|r| r.matches(&Version::parse("2.0.0").unwrap())));
        assert!(!reqs_hyphen_full.iter().any(|r| r.matches(&Version::parse("2.0.1").unwrap())));

        // Space separated bounds
        let reqs_spaces = parse_version_reqs(">= 1.0.0 < 2.0.0").unwrap();
        assert!(reqs_spaces.iter().any(|r| r.matches(&v1)));
        assert!(!reqs_spaces.iter().any(|r| r.matches(&Version::parse("2.0.0").unwrap())));

        // Wildcards & OR expressions
        let reqs_or = parse_version_reqs("1 - 3 || ^4.0.0").unwrap();
        assert!(reqs_or.iter().any(|r| r.matches(&v2)));
        assert!(reqs_or.iter().any(|r| r.matches(&v4)));
    }
}
