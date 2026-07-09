pub const DEFAULT_USER_AGENT: &str = "kumo/pm";


pub const DEFAULT_REGISTRY: &str = "npm";
pub const DEFAULT_REGISTRY_NPM_URL: &str = "https://registry.npmjs.org";
pub const DEFAULT_REGISTRY_KUMO_URL: &str = "https://kumo.unsetsoft.com";
pub const ENV_VAR_KUMO_REGISTRY: &str = "KUMO_REGISTRY";


pub const KUMO_DIR_NAME: &str = ".kumo";
pub const KUMO_CONFIG_JSON: &str = "kumo.config.json";
pub const KUMO_JSON: &str = "kumo.json";
pub const PACKAGE_JSON: &str = "package.json";
pub const KUMO_LOCK: &str = "kumo.lock";
pub const STORE_DIR_NAME: &str = "store";
pub const METADATA_DIR_NAME: &str = "metadata";
pub const OBJECTS_DIR_NAME: &str = "objects";
pub const SHIELD_STATE_FILE: &str = ".shield_state";
pub const CREDENTIALS_FILE: &str = "credentials.json";


pub const GITHUB_RELEASES_LIST_URL: &str = "https://api.github.com/repos/jmaxdev/kumo/releases";
pub const GITHUB_RELEASES_LATEST_URL: &str = "https://api.github.com/repos/jmaxdev/kumo/releases/latest";
pub const UPDATE_CHECK_INTERVAL_SECS: u64 = 86400;
pub const UPDATE_CHECK_TIMEOUT_SECS: u64 = 2;
pub const UPDATE_LAST_CHECK_FILE: &str = "last_check.json";


pub const RESOLVER_POOL_MAX_IDLE: usize = 20;
pub const RESOLVER_IDLE_TIMEOUT_SECS: u64 = 90;


pub const DEFAULT_POLICY_BLOCK_DEPRECATED: bool = true;
pub const DEFAULT_POLICY_MIN_SEVERITY: &str = "high";
pub const DEFAULT_POLICY_MINIMUM_RELEASE_AGE_MINS: u64 = 1440;
pub const DEFAULT_POLICY_ALLOW_POSTINSTALL: bool = false;
pub const DEFAULT_POLICY_TRUST_POLICY: &str = "none";
pub const DEFAULT_POLICY_TRUST_POLICY_IGNORE_AFTER_MINS: u64 = 10080;
pub const POPULAR_PACKAGES_API_URL: &str = "https://data.jsdelivr.com/v1/stats/packages";
pub const POPULAR_PACKAGES_CACHE_FILE: &str = "top_packages.json";
pub const POPULAR_PACKAGES_REFRESH_SECS: u64 = 7 * 24 * 60 * 60;
pub const POPULAR_PACKAGES_LIMIT: usize = 1000;
pub const OSV_API_QUERY_URL: &str = "https://api.osv.dev/v1/query";


pub const DEFAULT_ALLOWED_LICENSES: &[&str] = &["MIT", "Apache-2.0", "ISC", "BSD-3-Clause"];


pub const DEFAULT_ALLOWED_DOMAINS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "registry.npmjs.org",
    "nodejs.org",
    "localhost",
];


pub const DEFAULT_ALLOWED_IMPORT_HOSTS: &[&str] = &[];


pub const SANDBOX_DIR_NAME: &str = ".kumo_sandbox_home";
pub const SANDBOX_WINDOWS_PROXY_PORT: u16 = 9999;
pub const SANDBOX_WINDOWS_JOB_MEMORY_LIMIT: usize = 512 * 1024 * 1024;
pub const SANDBOX_MACOS_TEMP_SUBPATH: &str = "/private/tmp";


pub const PRIVATE_KEY_FILE: &str = "private_key.pem";
pub const PUBLIC_KEY_FILE: &str = "public_key.pem";

// Runtime management
pub const NODE_DIST_URL: &str = "https://nodejs.org/dist";
pub const NODE_DIST_INDEX_URL: &str = "https://nodejs.org/dist/index.json";
pub const RUNTIMES_DIR_NAME: &str = "runtimes";
pub const NODE_RUNTIME_DIR_NAME: &str = "node";
pub const RUNTIME_ACTIVE_FILE: &str = ".active";
pub const NODE_EOL_MAJOR_VERSION: u64 = 20;
