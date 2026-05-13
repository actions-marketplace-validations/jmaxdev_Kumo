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
    pub has_install_scripts: bool,
    pub bin: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TarballInfo {
    pub tarball: String,
    pub shasum: String,
    #[serde(rename = "unpackedSize", default)]
    pub size: u64,
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
            serde_json::from_str(&content)?
        } else {
            let url = format!("{}/{}", self.registry_url, name);
            let res: RegistryResponse = self
                .client
                .get(&url)
                .send()
                .await?
                .json()
                .await
                .with_context(|| format!("Failed to fetch metadata for {}", name))?;

            let json = serde_json::to_string(&res)?;
            let _ = std::fs::write(&cache_path, json);
            res
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

        let has_install_scripts = version_data.scripts.as_ref().map_or(false, |s| {
            s.contains_key("preinstall")
                || s.contains_key("install")
                || s.contains_key("postinstall")
        });

        let license = version_data.license.as_ref().and_then(|l| {
            if let Some(s) = l.as_str() {
                Some(s.to_string())
            } else {
                l.get("type").and_then(|t| t.as_str()).map(|s| s.to_string())
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
            has_install_scripts,
            bin: version_data.bin.clone(),
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
                if let Some(deps) = &metadata.dependencies {
                    for (d_name, d_range) in deps {
                        queue.push((d_name.clone(), d_range.clone()));
                    }
                }

                packages.insert(
                    key.clone(),
                    LockedPackage {
                        resolution: metadata.dist.clone(),
                        dependencies: metadata.dependencies.clone(),
                        bin: metadata.bin.clone(),
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
}
