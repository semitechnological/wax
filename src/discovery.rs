use crate::api::{Cask, Formula};
use crate::bottle::detect_platform;
use crate::cask::InstalledCask;
use crate::error::Result;
use crate::install::{InstallMode, InstalledPackage};
use crate::ui::dirs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{debug, info};

pub async fn discover_manual_casks(casks: &[Cask]) -> Result<HashMap<String, InstalledCask>> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = casks;
        return Ok(HashMap::new());
    }

    #[cfg(target_os = "macos")]
    {
        let alias_index = build_cask_alias_index(casks);
        let mut discovered = HashMap::new();

        for root in macos_app_roots() {
            if !root.exists() {
                continue;
            }

            let mut entries = match tokio::fs::read_dir(&root).await {
                Ok(entries) => entries,
                Err(err) => {
                    debug!("Skipping {:?}: {}", root, err);
                    continue;
                }
            };

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                if !path.is_dir() && !path.is_symlink() {
                    continue;
                }
                if !file_name.ends_with(".app") {
                    continue;
                }
                if file_name.starts_with('.') {
                    continue;
                }

                let bundle_name = app_bundle_name(&path)
                    .await
                    .unwrap_or_else(|| file_name.trim_end_matches(".app").to_string());

                let token = match_cask_token(&alias_index, &bundle_name)
                    .or_else(|| match_cask_token(&alias_index, &file_name));

                let Some(token) = token else {
                    continue;
                };

                let version = read_bundle_version(&path)
                    .await
                    .unwrap_or_else(|| "unknown".to_string());
                let install_date = entry
                    .metadata()
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(system_time_to_unix_secs)
                    .unwrap_or_else(now_unix_secs);

                discovered
                    .entry(token.clone())
                    .or_insert_with(|| InstalledCask {
                        name: token,
                        version,
                        install_date,
                        artifact_type: Some("app".to_string()),
                        binary_paths: None,
                        app_name: Some(bundle_name),
                    });
            }
        }

        if !discovered.is_empty() {
            info!(
                "Discovered {} manually installed cask(s) in /Applications",
                discovered.len()
            );
        }

        Ok(discovered)
    }
}

pub async fn discover_linux_formulae(
    formulae: &[Formula],
) -> Result<HashMap<String, InstalledPackage>> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = formulae;
        return Ok(HashMap::new());
    }

    #[cfg(target_os = "linux")]
    {
        let alias_index = build_formula_alias_index(formulae);
        let mut discovered = HashMap::new();

        for (name, version) in linux_package_inventory().await? {
            let Some(formula_name) = alias_index.get(&normalize_identifier(&name)).cloned() else {
                continue;
            };

            discovered
                .entry(formula_name.clone())
                .or_insert_with(|| InstalledPackage {
                    name: formula_name,
                    version,
                    platform: detect_platform(),
                    install_date: now_unix_secs(),
                    install_mode: InstallMode::Global,
                    from_source: false,
                    bottle_rebuild: 0,
                    bottle_sha256: None,
                    pinned: false,
                });
        }

        if !discovered.is_empty() {
            info!(
                "Discovered {} manually installed Linux package(s)",
                discovered.len()
            );
        }

        Ok(discovered)
    }
}

fn build_cask_alias_index(casks: &[Cask]) -> HashMap<String, String> {
    let mut index = HashMap::new();

    for cask in casks {
        for alias in cask_aliases(cask) {
            index
                .entry(normalize_identifier(&alias))
                .or_insert_with(|| cask.token.clone());
        }
    }

    index
}

fn build_formula_alias_index(formulae: &[Formula]) -> HashMap<String, String> {
    let mut index = HashMap::new();

    for formula in formulae {
        index
            .entry(normalize_identifier(&formula.name))
            .or_insert_with(|| formula.name.clone());
        index
            .entry(normalize_identifier(&formula.full_name))
            .or_insert_with(|| formula.name.clone());
    }

    index
}

fn cask_aliases(cask: &Cask) -> Vec<String> {
    let mut aliases = vec![cask.token.clone(), cask.full_token.clone()];
    aliases.extend(cask.name.clone());
    aliases
}

fn match_cask_token(alias_index: &HashMap<String, String>, value: &str) -> Option<String> {
    let normalized = normalize_identifier(value);
    if let Some(token) = alias_index.get(&normalized) {
        return Some(token.clone());
    }

    let stripped = value.trim_end_matches(".app");
    let normalized_stripped = normalize_identifier(stripped);
    alias_index.get(&normalized_stripped).cloned()
}

fn normalize_identifier(value: &str) -> String {
    let value = value
        .replace(".app", "")
        .replace("_", "-")
        .replace('/', "-")
        .to_lowercase();

    let mut out = String::new();
    let mut prev_dash = false;

    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else {
            Some('-')
        };

        if let Some(mapped) = mapped {
            if mapped == '-' {
                if !prev_dash && !out.is_empty() {
                    out.push(mapped);
                }
                prev_dash = true;
            } else {
                out.push(mapped);
                prev_dash = false;
            }
        }
    }

    out.trim_matches('-').to_string()
}

fn macos_app_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Ok(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    roots
}

async fn app_bundle_name(path: &Path) -> Option<String> {
    if let Some(name) = read_bundle_string(path, "CFBundleDisplayName").await {
        return Some(name);
    }
    if let Some(name) = read_bundle_string(path, "CFBundleName").await {
        return Some(name);
    }

    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

async fn read_bundle_version(path: &Path) -> Option<String> {
    if let Some(version) = read_bundle_string(path, "CFBundleShortVersionString").await {
        Some(version)
    } else {
        read_bundle_string(path, "CFBundleVersion").await
    }
}

async fn read_bundle_string(path: &Path, key: &str) -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        let _ = key;
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let plist = path.join("Contents/Info.plist");
        if !plist.exists() {
            return None;
        }

        let output = Command::new("plutil")
            .arg("-extract")
            .arg(key)
            .arg("raw")
            .arg("-o")
            .arg("-")
            .arg(&plist)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

async fn linux_package_inventory() -> Result<Vec<(String, String)>> {
    let mut inventories = Vec::new();

    if let Some(pkgs) = query_dpkg_packages().await? {
        inventories.extend(pkgs);
    }

    if inventories.is_empty() {
        if let Some(pkgs) = query_rpm_packages().await? {
            inventories.extend(pkgs);
        }
    }

    Ok(inventories)
}

async fn query_dpkg_packages() -> Result<Option<Vec<(String, String)>>> {
    let output = Command::new("dpkg-query")
        .arg("-W")
        .arg("-f=${binary:Package}\t${Version}\n")
        .output()
        .await;

    let Ok(output) = output else {
        return Ok(None);
    };

    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(parse_package_lines(&output.stdout, true)))
}

async fn query_rpm_packages() -> Result<Option<Vec<(String, String)>>> {
    let output = Command::new("rpm")
        .arg("-qa")
        .arg("--qf")
        .arg("%{NAME}\t%{VERSION}-%{RELEASE}\n")
        .output()
        .await;

    let Ok(output) = output else {
        return Ok(None);
    };

    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(parse_package_lines(&output.stdout, false)))
}

fn parse_package_lines(stdout: &[u8], strip_arch_suffix: bool) -> Vec<(String, String)> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let (name, version) = line.split_once('\t')?;
            let name = if strip_arch_suffix {
                name.split_once(':').map(|(base, _)| base).unwrap_or(name)
            } else {
                name
            };
            let name = name.trim();
            let version = version.trim();
            if name.is_empty() || version.is_empty() {
                None
            } else {
                Some((name.to_string(), version.to_string()))
            }
        })
        .collect()
}

fn system_time_to_unix_secs(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

fn now_unix_secs() -> i64 {
    system_time_to_unix_secs(SystemTime::now()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_common_app_names() {
        assert_eq!(normalize_identifier("Google Chrome.app"), "google-chrome");
        assert_eq!(
            normalize_identifier("Visual Studio Code"),
            "visual-studio-code"
        );
        assert_eq!(normalize_identifier("Docker Desktop"), "docker-desktop");
    }

    #[test]
    fn matches_cask_aliases() {
        let cask = Cask {
            token: "google-chrome".to_string(),
            full_token: "homebrew/cask/google-chrome".to_string(),
            name: vec!["Google Chrome".to_string()],
            desc: None,
            homepage: "https://www.google.com/chrome/".to_string(),
            version: "1.0".to_string(),
            deprecated: false,
            disabled: false,
        };
        let index = build_cask_alias_index(&[cask]);
        assert_eq!(
            match_cask_token(&index, "Google Chrome.app"),
            Some("google-chrome".to_string())
        );
        assert_eq!(
            match_cask_token(&index, "Google Chrome"),
            Some("google-chrome".to_string())
        );
    }

    #[test]
    fn parses_package_lines() {
        let input = b"vim\t2:9.1.0000-1\nchromium:amd64\t125.0.6422.141-1\n";
        let parsed = parse_package_lines(input, true);
        assert_eq!(parsed[0], ("vim".to_string(), "2:9.1.0000-1".to_string()));
        assert_eq!(
            parsed[1],
            ("chromium".to_string(), "125.0.6422.141-1".to_string())
        );
    }
}
