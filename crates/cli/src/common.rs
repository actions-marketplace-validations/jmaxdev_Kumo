use anyhow::Result;
use kumo_core::Store;
use reqwest;
use resolver::Resolver;
use security::{Policy, SecurityEngine};
use semver;
use serde_json;
use std::path::PathBuf;

pub async fn init_components() -> Result<(Store, SecurityEngine, Resolver)> {
    let store_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(kumo_core::config::KUMO_DIR_NAME)
        .join(kumo_core::config::STORE_DIR_NAME);

    let store = Store::new(store_path);
    store.init().await?;

    let mut config_json = serde_json::to_value(Policy::default())?;

    let global_config_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(kumo_core::config::KUMO_DIR_NAME)
        .join(kumo_core::config::KUMO_CONFIG_JSON);
    if let Ok(content) = std::fs::read_to_string(&global_config_path) {
        if let Ok(global_val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = global_val.as_object() {
                for (k, v) in obj {
                    let key = if k == "allowedImportHosts" || k == "AllowedImportHosts" || k == "allowed_import_hosts" {
                        "AllowedImportHost"
                    } else {
                        k.as_str()
                    };
                    config_json[key] = v.clone();
                }
            }
        }
    }

    let local_config_path = std::env::current_dir()?.join(kumo_core::config::KUMO_CONFIG_JSON);
    if let Ok(content) = std::fs::read_to_string(&local_config_path) {
        if let Ok(local_val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = local_val.as_object() {
                for (k, v) in obj {
                    let key = if k == "allowedImportHosts" || k == "AllowedImportHosts" || k == "allowed_import_hosts" {
                        "AllowedImportHost"
                    } else {
                        k.as_str()
                    };
                    config_json[key] = v.clone();
                }
            }
        }
    }

    let policy: Policy = serde_json::from_value(config_json)?;

    let mut security = SecurityEngine::new(policy);
    let resolver = Resolver::new();

    tokio::spawn(async move {});
    {
        let _ = security.refresh_popular_packages().await;
    }

    Ok((store, security, resolver))
}

pub fn get_deps_dir() -> String {
    let mut use_node_modules = false;
    let mut local_set = false;

    if let Ok(content) = std::fs::read_to_string(kumo_core::config::KUMO_CONFIG_JSON) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(b) = v["useNodeModules"].as_bool() {
                use_node_modules = b;
                local_set = true;
            }
        }
    }

    if !local_set {
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(kumo_core::config::KUMO_DIR_NAME).join(kumo_core::config::KUMO_CONFIG_JSON);
            if let Ok(content) = std::fs::read_to_string(global_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    use_node_modules = v["useNodeModules"].as_bool().unwrap_or(false);
                }
            }
        }
    }

    if use_node_modules || std::path::Path::new("node_modules").exists() {
        "node_modules".to_string()
    } else {
        "dependencies".to_string()
    }
}

pub async fn check_for_new_version() -> Option<String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let home = dirs::home_dir()?;
    let check_file = home.join(kumo_core::config::KUMO_DIR_NAME).join(kumo_core::config::UPDATE_LAST_CHECK_FILE);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let mut last_check = 0;
    let mut cached_latest = String::new();

    if check_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&check_file) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                last_check = v["timestamp"].as_u64().unwrap_or(0);
                cached_latest = v["version"].as_str().unwrap_or("").to_string();
            }
        }
    }

    if now - last_check > kumo_core::config::UPDATE_CHECK_INTERVAL_SECS || cached_latest.is_empty() {
        let client = reqwest::Client::builder()
            .user_agent(kumo_core::config::DEFAULT_USER_AGENT)
            .timeout(std::time::Duration::from_secs(kumo_core::config::UPDATE_CHECK_TIMEOUT_SECS))
            .build()
            .ok()?;

        let response = client
            .get(kumo_core::config::GITHUB_RELEASES_LATEST_URL)
            .send()
            .await
            .ok()?;

        if response.status().is_success() {
            if let Ok(release) = response.json::<serde_json::Value>().await {
                if let Some(tag) = release["tag_name"].as_str() {
                    cached_latest = tag.trim_start_matches('v').to_string();
                    let new_check_data = serde_json::json!({
                        "timestamp": now,
                        "version": cached_latest
                    });
                    let _ = std::fs::create_dir_all(check_file.parent().unwrap());
                    if let Ok(json) = serde_json::to_string(&new_check_data) {
                        let _ = std::fs::write(&check_file, json);
                    }
                }
            }
        }
    }

    if !cached_latest.is_empty() && cached_latest != current_version {
        let v_latest = semver::Version::parse(&cached_latest).ok()?;
        let v_current = semver::Version::parse(current_version).ok()?;
        if v_latest > v_current {
            return Some(cached_latest);
        }
    }

    None
}

#[allow(dead_code)]
pub async fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(&dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            Box::pin(copy_dir_recursive(
                &entry.path(),
                &dst.join(entry.file_name()),
            ))
            .await?;
        } else {
            tokio::fs::copy(entry.path(), dst.join(entry.file_name())).await?;
        }
    }
    Ok(())
}

pub async fn create_shim(
    bin_dir: &std::path::Path,
    name: &str,
    target: &std::path::Path,
) -> Result<()> {
    let deps_dir = bin_dir.parent().unwrap_or(bin_dir);
    if cfg!(target_os = "windows") {
        let shim_path = bin_dir.join(format!("{}.cmd", name));
        if let Some(parent) = shim_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = format!(
            "@ECHO OFF\nSET NODE_PATH={}\nnode \"{}\" %*",
            deps_dir.display(),
            target.display()
        );
        tokio::fs::write(shim_path, content).await?;
    } else {
        let shim_path = bin_dir.join(name);
        if let Some(parent) = shim_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = format!(
            "#!/bin/sh\nexport NODE_PATH=\"{}\"\nnode \"{}\" \"$@\"",
            deps_dir.display(),
            target.display()
        );
        tokio::fs::write(&shim_path, content).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&shim_path, perms)?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn ensure_kumo_polyfills() -> Result<String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let kumo_dir = home.join(kumo_core::config::KUMO_DIR_NAME);
    let lib_dir = kumo_dir.join("lib");
    if !lib_dir.exists() {
        std::fs::create_dir_all(&lib_dir)?;
    }

    let polyfill_path = lib_dir.join("api.mjs");
    let polyfill_content =
        include_str!("lib/api.mjs").replace("__KUMO_VERSION__", env!("CARGO_PKG_VERSION"));
    std::fs::write(&polyfill_path, polyfill_content)?;

    let loader_path = lib_dir.join("loader.mjs");
    let loader_content = include_str!("lib/loader.mjs");
    std::fs::write(&loader_path, loader_content)?;

    let dts_path = lib_dir.join("kumo.d.ts");
    let dts_content = include_str!("lib/kumo.d.ts");
    std::fs::write(&dts_path, dts_content)?;

    let mut polyfill_url = polyfill_path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !polyfill_url.starts_with('/') {
        polyfill_url = format!("/{}", polyfill_url);
    }

    Ok(polyfill_url)
}

pub fn print_update_banner(new_version: &str) {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("\n\x1b[33m┌─────────────────────────────────────────────────────────┐\x1b[0m");
    println!("\x1b[33m│\x1b[0m  New version of Kumo available: \x1b[32mv{}\x1b[0m -> \x1b[32mv{}\x1b[0m       \x1b[33m│\x1b[0m", current_version, new_version);
    println!("\x1b[33m│\x1b[0m  Run \x1b[36mkumo update\x1b[0m to upgrade!                          \x1b[33m│\x1b[0m");
    println!("\x1b[33m└─────────────────────────────────────────────────────────┘\x1b[0m\n");
}

pub fn parse_package_id(key: &str) -> (String, String) {
    if key.starts_with('@') {
        if let Some(version_at) = key[1..].find('@') {
            let split_idx = version_at + 1;
            let name = key[..split_idx].to_string();
            let version = key[split_idx + 1..].to_string();
            (name, version)
        } else {
            (key.to_string(), "unknown".to_string())
        }
    } else {
        if let Some(at_idx) = key.find('@') {
            let name = key[..at_idx].to_string();
            let version = key[at_idx + 1..].to_string();
            (name, version)
        } else {
            (key.to_string(), "unknown".to_string())
        }
    }
}

pub fn parse_package_arg(arg: &str) -> (String, String) {
    if arg.starts_with('@') {
        if let Some(second_at_idx) = arg[1..].find('@') {
            let split_idx = second_at_idx + 1;
            let name = arg[..split_idx].to_string();
            let version = arg[split_idx + 1..].to_string();
            (name, version)
        } else {
            (arg.to_string(), "latest".to_string())
        }
    } else {
        if let Some(at_idx) = arg.find('@') {
            let name = arg[..at_idx].to_string();
            let version = arg[at_idx + 1..].to_string();
            (name, version)
        } else {
            (arg.to_string(), "latest".to_string())
        }
    }
}

#[allow(dead_code)]
pub fn prepend_to_path(dir: &std::path::Path) -> String {
    let old_path = std::env::var("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    std::env::join_paths(paths)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            format!(
                "{}{}{}",
                dir.display(),
                if cfg!(windows) { ";" } else { ":" },
                old_path
            )
        })
}

#[allow(dead_code)]
fn package_has_types(current_dir: &std::path::Path, name: &str) -> bool {
    let deps_dir = get_deps_dir();
    let normalized_name = name.replace('/', std::path::MAIN_SEPARATOR_STR);
    let pkg_dir = current_dir.join(&deps_dir).join(&normalized_name);


    let types_name = if name.starts_with('@') {
        name.trim_start_matches('@').replace('/', "__")
    } else {
        name.to_string()
    };
    let types_dir = current_dir.join(&deps_dir).join("@types").join(types_name);
    if types_dir.exists() {
        return true;
    }

    if !pkg_dir.exists() {
        return false;
    }


    let pkg_json = pkg_dir.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val.get("types").is_some() || val.get("typings").is_some() {
                    return true;
                }

                if let Some(exports) = val.get("exports") {
                    let exports_str = exports.to_string();
                    if exports_str.contains(".d.ts") {
                        return true;
                    }
                }
            }
        }
    }


    if pkg_dir.join("index.d.ts").exists() {
        return true;
    }
    if pkg_dir.join("dist").join("index.d.ts").exists() {
        return true;
    }
    if pkg_dir.join("lib").join("index.d.ts").exists() {
        return true;
    }


    if let Ok(entries) = std::fs::read_dir(&pkg_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                if ext == "ts" && entry.path().to_string_lossy().ends_with(".d.ts") {
                    return true;
                }
            }
        }
    }

    false
}

#[allow(dead_code)]
pub fn update_kumo_dts() -> Result<()> {

    let current_dir = std::env::current_dir()?;
    let dot_kumo_dir = current_dir.join(kumo_core::config::KUMO_DIR_NAME);
    if !dot_kumo_dir.exists() {
        return Ok(());
    }

    let dts_path = dot_kumo_dir.join("kumo.d.ts");


    let mut dep_names = std::collections::BTreeSet::new();


    let pkg_json_path = current_dir.join(kumo_core::config::PACKAGE_JSON);
    if pkg_json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(deps) = val.get("dependencies").and_then(|d| d.as_object()) {
                    for k in deps.keys() {
                        dep_names.insert(k.clone());
                    }
                }
                if let Some(deps) = val.get("devDependencies").and_then(|d| d.as_object()) {
                    for k in deps.keys() {
                        dep_names.insert(k.clone());
                    }
                }
            }
        }
    }


    let kumo_json_path = current_dir.join(kumo_core::config::KUMO_JSON);
    if kumo_json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&kumo_json_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(deps) = val.get("dependencies").and_then(|d| d.as_object()) {
                    for k in deps.keys() {
                        dep_names.insert(k.clone());
                    }
                }
                if let Some(deps) = val.get("devDependencies").and_then(|d| d.as_object()) {
                    for k in deps.keys() {
                        dep_names.insert(k.clone());
                    }
                }
            }
        }
    }


    let lock_path = current_dir.join(kumo_core::config::KUMO_LOCK);
    if lock_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lock_path) {
            if let Ok(lf) = serde_yml::from_str::<resolver::Lockfile>(&content) {
                for key in lf.packages.keys() {
                    let (name, _) = parse_package_id(key);
                    dep_names.insert(name);
                }
            }
        }
    }

    let mut dts_content = include_str!("lib/kumo.d.ts").to_string();

    let mut fallback_decls = String::new();
    let mut url_decls = String::new();

    if !dep_names.is_empty() {
        for name in &dep_names {

            url_decls.push_str(&format!(
                "declare module \"https://*/{}\" {{\n    export * from \"{}\";\n    const _default: any;\n    export default _default;\n}}\n\n",
                name, name
            ));
            url_decls.push_str(&format!(
                "declare module \"http://*/{}\" {{\n    export * from \"{}\";\n    const _default: any;\n    export default _default;\n}}\n\n",
                name, name
            ));

            if !package_has_types(&current_dir, name) {
                fallback_decls.push_str(&format!(
                    "declare module \"{}\";\n",
                    name
                ));
            }
        }
    }

    if !fallback_decls.is_empty() {
        dts_content.push_str("\n// Fallback declarations for packages without built-in types\n");
        dts_content.push_str(&fallback_decls);
    }

    if !url_decls.is_empty() {
        dts_content.push_str("\n// Dynamic URL modules for project dependencies\n");
        dts_content.push_str(&url_decls);
    }

    std::fs::write(&dts_path, dts_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_package_id() {
        assert_eq!(
            parse_package_id("express@4.18.2"),
            ("express".to_string(), "4.18.2".to_string())
        );
        assert_eq!(
            parse_package_id("@types/node@14.14.31"),
            ("@types/node".to_string(), "14.14.31".to_string())
        );
        assert_eq!(
            parse_package_id("lodash"),
            ("lodash".to_string(), "unknown".to_string())
        );
        assert_eq!(
            parse_package_id("@nestjs/core"),
            ("@nestjs/core".to_string(), "unknown".to_string())
        );
    }

    #[test]
    fn test_parse_package_arg() {
        assert_eq!(
            parse_package_arg("express@4.18.2"),
            ("express".to_string(), "4.18.2".to_string())
        );
        assert_eq!(
            parse_package_arg("@types/node@14.14.31"),
            ("@types/node".to_string(), "14.14.31".to_string())
        );
        assert_eq!(
            parse_package_arg("lodash"),
            ("lodash".to_string(), "latest".to_string())
        );
        assert_eq!(
            parse_package_arg("@nestjs/core"),
            ("@nestjs/core".to_string(), "latest".to_string())
        );
    }

    #[test]
    fn test_preserve_json_key_order() {
        let input_json = r#"{
  "name": "kumo-dep-test",
  "version": "1.0.6",
  "dependencies": {
    "vite": "^8.0.16"
  },
  "author": ""
}"#;
        let mut val: serde_json::Value = serde_json::from_str(input_json).unwrap();
        if let Some(deps) = val.get_mut("dependencies").and_then(|d| d.as_object_mut()) {
            deps.insert("express".to_string(), serde_json::json!("^4.18.2"));
        }
        let keys: Vec<&str> = val
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(keys, vec!["name", "version", "dependencies", "author"]);
    }

    #[test]
    fn test_policy_allowed_import_hosts_deserialization() {
        let json_data = r#"{
            "AllowedImportHost": ["esm.sh", "cdn.jsdelivr.net"]
        }"#;
        let policy: Policy = serde_json::from_str(json_data).unwrap();
        assert!(policy.allowed_import_hosts.contains("esm.sh"));
        assert!(policy.allowed_import_hosts.contains("cdn.jsdelivr.net"));
        assert_eq!(policy.allowed_import_hosts.len(), 2);


        let json_data_alias1 = r#"{
            "allowedImportHosts": ["unpkg.com"]
        }"#;
        let policy_alias1: Policy = serde_json::from_str(json_data_alias1).unwrap();
        assert!(policy_alias1.allowed_import_hosts.contains("unpkg.com"));


        let policy_default = Policy::default();
        let serialized = serde_json::to_string(&policy_default).unwrap();
        assert!(serialized.contains("\"AllowedImportHost\":"));
    }

    #[test]
    fn test_package_has_types() {
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("kumo_test_types_{}", timestamp));
        let _ = std::fs::create_dir_all(&temp_dir);

        let config_path = temp_dir.join("kumo.config.json");
        std::fs::write(&config_path, r#"{"useNodeModules": true}"#).unwrap();

        let node_modules_dir = temp_dir.join("node_modules");
        std::fs::create_dir_all(&node_modules_dir).unwrap();


        let pkg_no_types = node_modules_dir.join("pkg-no-types");
        std::fs::create_dir_all(&pkg_no_types).unwrap();
        std::fs::write(pkg_no_types.join("package.json"), r#"{"name": "pkg-no-types"}"#).unwrap();
        std::fs::write(pkg_no_types.join("index.js"), "module.exports = {};").unwrap();


        let pkg_with_dts = node_modules_dir.join("pkg-with-dts");
        std::fs::create_dir_all(&pkg_with_dts).unwrap();
        std::fs::write(pkg_with_dts.join("package.json"), r#"{"name": "pkg-with-dts"}"#).unwrap();
        std::fs::write(pkg_with_dts.join("index.d.ts"), "export declare const foo: string;").unwrap();


        let pkg_with_types_field = node_modules_dir.join("pkg-with-types-field");
        std::fs::create_dir_all(&pkg_with_types_field).unwrap();
        std::fs::write(pkg_with_types_field.join("package.json"), r#"{"name": "pkg-with-types-field", "types": "./types.d.ts"}"#).unwrap();
        std::fs::write(pkg_with_types_field.join("types.d.ts"), "export declare const bar: number;").unwrap();


        let pkg_with_external_types = node_modules_dir.join("pkg-with-external-types");
        std::fs::create_dir_all(&pkg_with_external_types).unwrap();
        std::fs::write(pkg_with_external_types.join("package.json"), r#"{"name": "pkg-with-external-types"}"#).unwrap();

        let external_types_dir = node_modules_dir.join("@types").join("pkg-with-external-types");
        std::fs::create_dir_all(&external_types_dir).unwrap();
        std::fs::write(external_types_dir.join("index.d.ts"), "export declare const baz: boolean;").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let has_types_1 = package_has_types(&temp_dir, "pkg-no-types");
        let has_types_2 = package_has_types(&temp_dir, "pkg-with-dts");
        let has_types_3 = package_has_types(&temp_dir, "pkg-with-types-field");
        let has_types_4 = package_has_types(&temp_dir, "pkg-with-external-types");

        std::env::set_current_dir(original_dir).unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert!(!has_types_1);
        assert!(has_types_2);
        assert!(has_types_3);
        assert!(has_types_4);
    }
}
