use crate::bottle::homebrew_prefix;
use crate::cache::Cache;
use crate::cask::CaskState;
use crate::commands::upgrade::get_outdated_packages;
use crate::error::Result;
use crate::install::InstallState;
use console::style;
use tracing::instrument;

#[instrument(skip(cache))]
pub async fn list(cache: Option<&Cache>, upgradable: bool) -> Result<()> {
    // When --upgradable is set, delegate to the outdated view (with upgrade arrows)
    if upgradable {
        let cache = match cache {
            Some(c) => c,
            None => {
                println!("no packages installed");
                return Ok(());
            }
        };
        let outdated = get_outdated_packages(cache).await?;
        if outdated.is_empty() {
            println!("all packages are up to date");
            return Ok(());
        }
        println!();
        for pkg in &outdated {
            let tag = if pkg.is_cask {
                format!(" {}", style("(cask)").yellow())
            } else {
                String::new()
            };
            println!(
                "{}{} {} → {}",
                style(&pkg.name).magenta(),
                tag,
                style(&pkg.installed_version).dim(),
                style(&pkg.latest_version).green()
            );
        }
        println!(
            "\n{} package{} can be upgraded",
            style(outdated.len()).cyan(),
            if outdated.len() == 1 { "" } else { "s" }
        );
        return Ok(());
    }

    let candidates = [
        homebrew_prefix().join("Cellar"),
        crate::ui::dirs::home_dir()
            .unwrap_or_else(|_| homebrew_prefix())
            .join(".local/wax/Cellar"),
    ];

    let cellar_path = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| homebrew_prefix().join("Cellar"));

    let cask_state = CaskState::new()?;
    let installed_casks = cask_state.load().await?;

    let install_state = InstallState::new()?;
    let installed_packages = install_state.load().await?;

    let mut packages = Vec::new();

    if cellar_path.exists() {
        let mut entries = tokio::fs::read_dir(&cellar_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let package_name = entry.file_name().to_string_lossy().to_string();

                let mut versions = Vec::new();
                let mut version_entries = tokio::fs::read_dir(entry.path()).await?;
                while let Some(version_entry) = version_entries.next_entry().await? {
                    if version_entry.file_type().await?.is_dir() {
                        versions.push(version_entry.file_name().to_string_lossy().to_string());
                    }
                }

                let pkg_meta = installed_packages.get(&package_name);
                let from_source = pkg_meta.map(|p| p.from_source).unwrap_or(false);
                let pinned = pkg_meta.map(|p| p.pinned).unwrap_or(false);

                packages.push((package_name, versions, from_source, pinned));
            }
        }
    }

    if packages.is_empty() && installed_casks.is_empty() {
        println!("no packages installed");
        return Ok(());
    }

    println!();

    if !packages.is_empty() {
        packages.sort_by(|a, b| a.0.cmp(&b.0));

        for (package, versions, from_source, pinned) in &packages {
            let version_str = versions.join(", ");
            let pin_marker = if *pinned {
                format!(" {}", style("(pinned)").cyan())
            } else {
                String::new()
            };
            let src_marker = if *from_source {
                format!(" {}", style("(source)").yellow())
            } else {
                String::new()
            };
            println!(
                "{} {}{}{}",
                style(package).magenta(),
                style(&version_str).dim(),
                src_marker,
                pin_marker
            );
        }
    }

    if !installed_casks.is_empty() {
        let mut cask_list: Vec<_> = installed_casks.iter().collect();
        cask_list.sort_by_key(|(name, _)| *name);

        for (cask_name, cask) in cask_list {
            println!(
                "{} {} {}",
                style(cask_name).magenta(),
                style(&cask.version).dim(),
                style("(cask)").yellow()
            );
        }
    }

    let total = packages.len() + installed_casks.len();
    let parts: Vec<String> = [
        if packages.is_empty() {
            None
        } else {
            Some(format!(
                "{} {}",
                packages.len(),
                if packages.len() == 1 {
                    "formula"
                } else {
                    "formulae"
                }
            ))
        },
        if installed_casks.is_empty() {
            None
        } else {
            Some(format!(
                "{} {}",
                installed_casks.len(),
                if installed_casks.len() == 1 {
                    "cask"
                } else {
                    "casks"
                }
            ))
        },
    ]
    .into_iter()
    .flatten()
    .collect();

    println!(
        "\n{} {} installed ({})",
        style(total).cyan(),
        if total == 1 { "package" } else { "packages" },
        parts.join(", ")
    );

    Ok(())
}
