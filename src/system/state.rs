use crate::error::{Result, WaxError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

fn state_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| WaxError::InstallError("HOME not set".into()))?;
    Ok(PathBuf::from(home).join(".wax").join("system").join("state.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: Option<String>,
    pub installed_at: i64,
    /// Whether this package was explicitly declared by the user (vs. a dep)
    pub declared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemState {
    /// Packages currently tracked as installed.
    pub installed: HashMap<String, InstalledPackage>,
    /// User-declared packages (the desired set, like a packages list).
    pub declared: Vec<String>,
}

impl SystemState {
    pub async fn load() -> Result<Self> {
        let path = state_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = tokio::fs::read_to_string(&path).await?;
        let state: Self = serde_json::from_str(&raw)?;
        Ok(state)
    }

    pub async fn save(&self) -> Result<()> {
        let path = state_path()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, raw).await?;
        Ok(())
    }

    pub fn mark_installed(&mut self, name: &str, version: Option<String>, declared: bool) {
        self.installed.insert(
            name.to_string(),
            InstalledPackage {
                name: name.to_string(),
                version,
                installed_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                declared,
            },
        );
    }

    pub fn mark_removed(&mut self, name: &str) {
        self.installed.remove(name);
    }

    pub fn declare(&mut self, name: &str) {
        if !self.declared.iter().any(|p| p == name) {
            self.declared.push(name.to_string());
        }
        if let Some(pkg) = self.installed.get_mut(name) {
            pkg.declared = true;
        }
    }

    pub fn undeclare(&mut self, name: &str) {
        self.declared.retain(|p| p != name);
        if let Some(pkg) = self.installed.get_mut(name) {
            pkg.declared = false;
        }
    }
}
