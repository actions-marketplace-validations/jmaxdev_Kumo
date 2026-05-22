use semver::Version;
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
    pub os: Option<Vec<String>>,
    pub cpu: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RegistrySignature {
    pub keyid: String,
    pub sig: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RegistryAttestations {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TarballInfo {
    pub tarball: String,
    pub shasum: String,
    #[serde(default)]
    pub size: u64,
    #[serde(rename = "unpackedSize", default)]
    pub unpacked_size: u64,
    #[serde(default)]
    pub signatures: Option<Vec<RegistrySignature>>,
    #[serde(rename = "npm-signature", default)]
    pub npm_signature: Option<String>,
    #[serde(default)]
    pub attestations: Option<RegistryAttestations>,
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
pub struct RegistryResponse {
    pub name: String,
    pub versions: HashMap<String, RegistryVersion>,
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    pub time: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryVersion {
    pub name: String,
    pub version: String,
    pub dependencies: Option<HashMap<String, String>>,
    pub dist: TarballInfo,
    pub license: Option<serde_json::Value>,
    pub deprecated: Option<serde_json::Value>,
    pub scripts: Option<HashMap<String, String>>,
    pub bin: Option<serde_json::Value>,
    #[serde(rename = "optionalDependencies")]
    pub optional_dependencies: Option<HashMap<String, String>>,
    pub os: Option<Vec<String>>,
    pub cpu: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lockfile {
    pub lockfile_version: String,
    pub config_hash: Option<String>,
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
    #[serde(default)]
    pub published_at: Option<String>,
}
