use anyhow::{anyhow, Context, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageMetadata {
    pub name: String,
    pub version: Version,
    pub dependencies: Option<HashMap<String, String>>,
    pub dist: TarballInfo,
    pub license: Option<String>,
    pub deprecated: Option<String>,
    pub published_at: Option<String>,
    pub bin: Option<serde_json::Value>,
    pub scripts: Option<HashMap<String, String>>,
    pub optional_dependencies: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TarballInfo {
    pub tarball: String,
    pub shasum: String,
    #[serde(default)]
    pub size: u64,
    #[serde(rename = "unpackedSize", default)]
    pub unpacked_size: u64,
}

impl TarballInfo {
    pub fn get_size(&self) -> u64 {
        if self.unpacked_size > 0 {
            self.unpacked_size
        } else {
            self.size
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryResponse {
    name: String,
    versions: HashMap<String, RegistryVersion>,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    time: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryVersion {
    name: String,
    version: String,
    dependencies: Option<HashMap<String, String>>,
    dist: TarballInfo,
    license: Option<serde_json::Value>,
    deprecated: Option<serde_json::Value>,
    scripts: Option<HashMap<String, String>>,
    bin: Option<serde_json::Value>,
    #[serde(rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lockfile {
    pub lockfile_version: String,
    pub dependencies: HashMap<String, String>,
    pub packages: HashMap<String, LockedPackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockedPackage {
    pub resolution: TarballInfo,
    pub dependencies: Option<HashMap<String, String>>,
    pub bin: Option<serde_json::Value>,
    pub scripts: Option<HashMap<String, String>>,
    pub optional_dependencies: Option<HashMap<String, String>>,
}

pub struct Resolver {
    client: reqwest::Client,
    registry_url: String,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            registry_url: "https://registry.npmjs.org".to_string(),
        }
    }

    pub async fn resolve_package(&self, name: &str, range: &str) -> Result<PackageMetadata> {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".kumo")
            .join("cache")
            .join("metadata");

        let _ = std::fs::create_dir_all(&cache_dir);
        let cache_path = cache_dir.join(format!("{}.json", name.replace('/', "__")));

        let response: RegistryResponse = if cache_path.exists() {
            let content = std::fs::read_to_string(&cache_path)?;
            let res: RegistryResponse = serde_json::from_str(&content)?;
            // If the first version in metadata is missing optional_dependencies field (as Option), 
            // it might be an old cache. We re-fetch to be sure.
            let is_old_cache = res.versions.values().next().map_or(true, |v| v.optional_dependencies.is_none());
            if res.versions.is_empty() || is_old_cache {
                self.fetch_and_cache_metadata(name, &cache_path).await?
            } else {
                res
            }
        } else {
            self.fetch_and_cache_metadata(name, &cache_path).await?
        };

        let version_str = if range == "latest" || range == "*" || range == "" {
            response
                .dist_tags
                .get("latest")
                .ok_or_else(|| anyhow!("No latest tag found for {}", name))?
                .to_string()
        } else {
            let reqs: Vec<VersionReq> = range
                .split("||")
                .map(|r| {
                    let normalized = r
                        .trim()
                        .replace(">= ", ">=")
                        .replace("<= ", "<=")
                        .replace("> ", ">")
                        .replace("< ", "<")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(",");
                    VersionReq::parse(&normalized)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| anyhow!("Invalid semver range: {}", range))?;

            let mut versions: Vec<Version> = response
                .versions
                .keys()
                .filter_map(|v| Version::parse(v).ok())
                .filter(|v| reqs.iter().any(|req| req.matches(v)))
                .collect();

            versions.sort();
            versions
                .last()
                .map(|v| v.to_string())
                .ok_or_else(|| anyhow!("No version matching {} found for {}", range, name))?
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
        })
    }

    pub async fn resolve_tree(&self, root_deps: &HashMap<String, String>) -> Result<Lockfile> {
        let mut packages = HashMap::new();
        let mut resolved_root_deps = HashMap::new();
        let mut queue: Vec<(String, String)> = root_deps
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        while let Some((name, range)) = queue.pop() {
            let metadata = self.resolve_package(&name, &range).await?;
            let key = format!("{}@{}", metadata.name, metadata.version);

            if !packages.contains_key(&key) {
                let mut all_deps = metadata.dependencies.clone().unwrap_or_default();
                if let Some(opt_deps) = &metadata.optional_dependencies {
                    for (k, v) in opt_deps {
                        all_deps.insert(k.clone(), v.clone());
                    }
                }

                for (d_name, d_range) in all_deps {
                    queue.push((d_name.clone(), d_range.clone()));
                }

                packages.insert(
                    key.clone(),
                    LockedPackage {
                        resolution: metadata.dist.clone(),
                        dependencies: metadata.dependencies.clone(),
                        bin: metadata.bin.clone(),
                        scripts: metadata.scripts.clone(),
                        optional_dependencies: metadata.optional_dependencies.clone(),
                    },
                );
            }

            if root_deps.contains_key(&name) {
                resolved_root_deps.insert(name, metadata.version.to_string());
            }
        }
        Ok(Lockfile {
            lockfile_version: "1.0".to_string(),
            dependencies: resolved_root_deps,
            packages,
        })
    }

    async fn fetch_and_cache_metadata(&self, name: &str, cache_path: &std::path::Path) -> Result<RegistryResponse> {
        let url = format!("{}/{}", self.registry_url, name);
        let mut last_err = None;

        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt)).await;
            }

            match self.client.get(&url).send().await {
                Ok(res) => {
                    let metadata: RegistryResponse = res.json().await.with_context(|| format!("Failed to parse metadata for {}", name))?;
                    let json = serde_json::to_string(&metadata)?;
                    let _ = std::fs::write(cache_path, json);
                    return Ok(metadata);
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(anyhow!("Failed to fetch metadata for {} after 3 attempts: {:?}", name, last_err))
    }
}
