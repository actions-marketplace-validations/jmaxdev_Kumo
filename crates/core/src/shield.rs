use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;

#[derive(Clone)]
pub struct ShieldManager {
    state_path: PathBuf,
}

impl ShieldManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let state_path = home.join(".kumo").join(".shield_state");
        Self { state_path }
    }

    pub fn is_active(&self) -> bool {
        if !self.state_path.exists() {
            return false;
        }
        if let Ok(content) = fs::read_to_string(&self.state_path) {
            return content.trim() == "on";
        }
        false
    }

    pub fn set_active(&self, active: bool) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // VULN-3: Symlink attack prevention
        if let Ok(metadata) = fs::symlink_metadata(&self.state_path) {
            if metadata.file_type().is_symlink() {
                let _ = fs::remove_file(&self.state_path);
            }
        }

        let state_str = if active { "on" } else { "off" };
        
        // VULN-5: Atomic write
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = self.state_path.with_extension(format!("tmp-{}-{}", std::process::id(), now));
        
        fs::write(&tmp_path, state_str).context("Failed to write shield state temp file")?;
        fs::rename(&tmp_path, &self.state_path).context("Failed to atomically rename shield state")?;

        Ok(())
    }

    pub fn shield_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            // We do not shield the symlink itself, or we might follow it and shield unintended target
            return Ok(());
        }
        let mut perms = metadata.permissions();
        if !perms.readonly() {
            perms.set_readonly(true);
            fs::set_permissions(path, perms)?;
        }
        Ok(())
    }

    pub fn unshield_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Ok(());
        }
        let mut perms = metadata.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            fs::set_permissions(path, perms)?;
        }
        Ok(())
    }

    pub fn unshield_dir_recursive(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        
        let metadata = fs::symlink_metadata(path)?;
        let mut perms = metadata.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }

        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).min_depth(1) {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    if let Ok(meta) = fs::symlink_metadata(entry_path) {
                        let mut p = meta.permissions();
                        if p.readonly() {
                            p.set_readonly(false);
                            let _ = fs::set_permissions(entry_path, p);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}
