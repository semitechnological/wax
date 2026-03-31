use super::{PackageIndex, PackageMetadata};
use crate::error::{Result, WaxError};
use flate2::read::GzDecoder;
use std::io::Read;
use std::time::{Duration, SystemTime};
use tracing::{debug, warn};

pub struct AptRegistry {
    mirror: String,
    suite: String,
    components: Vec<String>,
    arch: String,
}

impl AptRegistry {
    pub fn new(mirror: &str, suite: &str) -> Self {
        let arch = std::env::consts::ARCH;
        let deb_arch = match arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "arm" => "armhf",
            _ => "amd64",
        };
        Self {
            mirror: mirror.to_string(),
            suite: suite.to_string(),
            components: vec!["main".to_string(), "universe".to_string()],
            arch: deb_arch.to_string(),
        }
    }

    pub fn ubuntu_default() -> Self {
        Self::new("http://archive.ubuntu.com/ubuntu", "jammy")
    }

    pub fn debian_default() -> Self {
        Self::new("http://deb.debian.org/debian", "bookworm")
    }

    fn cache_path(&self) -> Result<std::path::PathBuf> {
        let dir = crate::ui::dirs::wax_cache_dir()?.join("system");
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("apt-{}.json", self.suite)))
    }

    fn is_cache_fresh(path: &std::path::Path) -> bool {
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                    return elapsed < Duration::from_secs(24 * 3600);
                }
            }
        }
        false
    }

    pub async fn load(&self, client: &reqwest::Client) -> Result<PackageIndex> {
        let cache_path = self.cache_path()?;

        if Self::is_cache_fresh(&cache_path) {
            debug!("Loading APT index from cache: {:?}", cache_path);
            let data = std::fs::read_to_string(&cache_path)?;
            let packages: Vec<PackageMetadata> = serde_json::from_str(&data)?;
            return Ok(PackageIndex { packages });
        }

        debug!(
            "Fetching APT index for {} suite={} arch={}",
            self.mirror, self.suite, self.arch
        );

        let mut all_packages: Vec<PackageMetadata> = Vec::new();

        for component in &self.components {
            let url = format!(
                "{}/dists/{}/{}/binary-{}/Packages.gz",
                self.mirror, self.suite, component, self.arch
            );
            debug!("Fetching {}", url);

            let resp = client.get(&url).send().await.map_err(|e| {
                WaxError::InstallError(format!("Failed to fetch APT index from {}: {}", url, e))
            })?;

            if !resp.status().is_success() {
                warn!(
                    "APT index fetch failed for component {}: HTTP {}",
                    component,
                    resp.status()
                );
                continue;
            }

            let bytes = resp.bytes().await.map_err(|e| {
                WaxError::InstallError(format!("Failed to read APT index body: {}", e))
            })?;

            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = String::new();
            decoder.read_to_string(&mut decompressed).map_err(|e| {
                WaxError::InstallError(format!("Failed to decompress APT Packages.gz: {}", e))
            })?;

            let pkgs = parse_packages_file(&decompressed, &self.mirror);
            debug!(
                "Parsed {} packages from {}/{}",
                pkgs.len(),
                self.suite,
                component
            );
            all_packages.extend(pkgs);
        }

        // Deduplicate by name, keeping first seen
        let mut seen = std::collections::HashSet::new();
        all_packages.retain(|p| seen.insert(p.name.clone()));

        let json = serde_json::to_string(&all_packages)?;
        std::fs::write(&cache_path, &json)?;

        Ok(PackageIndex {
            packages: all_packages,
        })
    }
}

fn parse_packages_file(content: &str, mirror: &str) -> Vec<PackageMetadata> {
    let mut packages = Vec::new();

    for stanza in content.split("\n\n") {
        let stanza = stanza.trim();
        if stanza.is_empty() {
            continue;
        }

        let mut name = String::new();
        let mut version = String::new();
        let mut description = String::new();
        let mut filename = String::new();
        let mut sha256: Option<String> = None;
        let mut installed_size: u64 = 0;
        let mut depends: Vec<String> = Vec::new();
        let mut provides: Vec<String> = Vec::new();

        let mut current_key = String::new();
        let mut current_val = String::new();

        let mut flush = |key: &str, val: &str| {
            let val = val.trim();
            match key {
                "Package" => name = val.to_string(),
                "Version" => version = val.to_string(),
                "Description" => description = val.lines().next().unwrap_or(val).to_string(),
                "Filename" => filename = val.to_string(),
                "SHA256" => sha256 = Some(val.to_string()),
                "Installed-Size" => {
                    installed_size = val.parse::<u64>().unwrap_or(0) * 1024;
                }
                "Depends" => {
                    for dep in val.split(',') {
                        let dep = dep.trim();
                        // Handle alternatives with |
                        let primary = dep.split('|').next().unwrap_or(dep).trim();
                        if !primary.is_empty() {
                            depends.push(super::parse_dep_name(primary).to_string());
                        }
                    }
                }
                "Provides" => {
                    for p in val.split(',') {
                        let pname = super::parse_dep_name(p.trim());
                        if !pname.is_empty() {
                            provides.push(pname.to_string());
                        }
                    }
                }
                _ => {}
            }
        };

        for line in stanza.lines() {
            if line.starts_with(' ') || line.starts_with('\t') {
                // Continuation line
                current_val.push('\n');
                current_val.push_str(line.trim_start());
            } else if let Some(colon_pos) = line.find(':') {
                // New field: flush current
                if !current_key.is_empty() {
                    flush(&current_key.clone(), &current_val.clone());
                }
                current_key = line[..colon_pos].trim().to_string();
                current_val = line[colon_pos + 1..].trim().to_string();
            }
        }
        if !current_key.is_empty() {
            flush(&current_key.clone(), &current_val.clone());
        }

        if name.is_empty() || version.is_empty() || filename.is_empty() {
            continue;
        }

        let download_url = format!("{}/{}", mirror, filename);

        packages.push(PackageMetadata {
            name,
            version,
            description,
            download_url,
            sha256,
            installed_size,
            depends,
            provides,
        });
    }

    packages
}
