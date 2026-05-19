use std::path::{Path, PathBuf};
use std::process::Command;

pub struct SandboxConfig {
    pub allow_network: bool,
    pub pkg_dir: PathBuf,
}

pub struct SandboxRunner;

impl SandboxRunner {
    pub fn create_command(pkg_dir: &Path, script: &str, allow_network: bool) -> Command {
        #[cfg(target_os = "linux")]
        {
            Self::create_linux(pkg_dir, script, allow_network)
        }

        #[cfg(target_os = "macos")]
        {
            Self::create_macos(pkg_dir, script, allow_network)
        }

        #[cfg(target_os = "windows")]
        {
            Self::create_windows(pkg_dir, script, allow_network)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::create_fallback(pkg_dir, script, allow_network)
        }
    }

    fn setup_env(cmd: &mut Command, pkg_dir: &Path) {
        let fake_home = pkg_dir.join(".kumo_sandbox_home");
        let _ = std::fs::create_dir_all(&fake_home);

        cmd.env("HOME", &fake_home);
        cmd.env("USERPROFILE", &fake_home);
        cmd.env("APPDATA", fake_home.join("AppData"));
        cmd.env("LOCALAPPDATA", fake_home.join("LocalAppData"));

        let sensitive_vars = [
            "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN", "AWS_DEFAULT_REGION",
            "GCP_PROJECT", "GOOGLE_APPLICATION_CREDENTIALS",
            "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID",
            "GITHUB_TOKEN", "GITHUB_AUTH_TOKEN", "NPM_TOKEN", "NPM_AUTH_TOKEN",
            "STRIPE_API_KEY", "SENDGRID_API_KEY", "SLACK_BOT_TOKEN", "DISCORD_TOKEN"
        ];
        for var in &sensitive_vars {
            cmd.env_remove(var);
        }
    }

    #[cfg(target_os = "linux")]
    fn create_linux(pkg_dir: &Path, script: &str, allow_network: bool) -> Command {
        let has_bwrap = Command::new("which")
            .arg("bwrap")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !has_bwrap {
            return Self::create_fallback(pkg_dir, script, allow_network);
        }

        let mut cmd = Command::new("bwrap");
        cmd.arg("--unshare-all");
        if !allow_network {
            cmd.arg("--unshare-net");
        }

        cmd.arg("--ro-bind").arg("/usr").arg("/usr")
           .arg("--ro-bind").arg("/lib").arg("/lib")
           .arg("--ro-bind").arg("/lib64").arg("/lib64")
           .arg("--ro-bind").arg("/bin").arg("/bin")
           .arg("--ro-bind").arg("/sbin").arg("/sbin")
           .arg("--ro-bind").arg("/etc").arg("/etc")
           .arg("--proc").arg("/proc")
           .arg("--dev").arg("/dev");

        cmd.arg("--bind").arg(pkg_dir).arg(pkg_dir);
        cmd.arg("--chdir").arg(pkg_dir);

        cmd.arg("sh").arg("-c").arg(script);

        Self::setup_env(&mut cmd, pkg_dir);

        cmd
    }

    #[cfg(target_os = "macos")]
    fn create_macos(pkg_dir: &Path, script: &str, allow_network: bool) -> Command {
        let mut cmd = Command::new("sandbox-exec");
        let network_rule = if allow_network {
            "(allow network*)"
        } else {
            "(deny network*)"
        };

        let profile = format!(
            r#"
            (version 1)
            (deny default)
            (allow process-fork)
            (allow sysctl-read)
            (allow file-read*)
            {}
            (allow file-write* (subpath "/private/tmp"))
            (allow file-write* (subpath "{}"))
            "#,
            network_rule,
            pkg_dir.to_string_lossy()
        );

        cmd.arg("-p").arg(profile);
        cmd.arg("sh").arg("-c").arg(script);
        cmd.current_dir(pkg_dir);

        Self::setup_env(&mut cmd, pkg_dir);

        cmd
    }

    #[cfg(target_os = "windows")]
    fn create_windows(pkg_dir: &Path, script: &str, allow_network: bool) -> Command {
        let mut cmd = Command::new("cmd");
        cmd.arg("/c").arg(script);
        cmd.current_dir(pkg_dir);

        Self::setup_env(&mut cmd, pkg_dir);

        if !allow_network {
            cmd.env("HTTP_PROXY", "http://127.0.0.1:9999");
            cmd.env("HTTPS_PROXY", "http://127.0.0.1:9999");
            cmd.env("NODE_TLS_REJECT_UNAUTHORIZED", "0");
        }

        cmd
    }

    #[allow(dead_code)]
    fn create_fallback(pkg_dir: &Path, script: &str, allow_network: bool) -> Command {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/c").arg(script);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(script);
            c
        };

        cmd.current_dir(pkg_dir);
        Self::setup_env(&mut cmd, pkg_dir);

        if !allow_network {
            cmd.env("HTTP_PROXY", "http://127.0.0.1:9999");
            cmd.env("HTTPS_PROXY", "http://127.0.0.1:9999");
            cmd.env("NODE_TLS_REJECT_UNAUTHORIZED", "0");
        }

        cmd
    }
}
