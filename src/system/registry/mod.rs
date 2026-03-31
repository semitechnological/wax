pub mod apt;
pub mod apk;
pub mod pacman;
pub mod dnf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub download_url: String,
    pub sha256: Option<String>,
    pub installed_size: u64,
    pub depends: Vec<String>,
    pub provides: Vec<String>,
}

pub struct PackageIndex {
    pub packages: Vec<PackageMetadata>,
}

impl PackageIndex {
    pub fn find(&self, name: &str) -> Option<&PackageMetadata> {
        self.packages
            .iter()
            .find(|p| p.name == name)
            .or_else(|| {
                self.packages
                    .iter()
                    .find(|p| p.provides.iter().any(|prov| prov == name))
            })
    }
}

/// Strip version constraints from a dep string like "libc6 (>= 2.17)" → "libc6"
pub fn parse_dep_name(dep: &str) -> &str {
    dep.split_whitespace()
        .next()
        .unwrap_or(dep)
        .split('(')
        .next()
        .unwrap_or(dep)
        .trim()
}
