use crate::error::{Result, WaxError};
use crate::install::{create_symlinks, InstallState};
use console::style;

pub async fn link(packages: &[String], all: bool) -> Result<()> {
    let state = InstallState::new()?;
    state.sync_from_cellar().await.ok();
    let installed = state.load().await?;

    if installed.is_empty() {
        println!("no packages installed");
        return Ok(());
    }

    let packages_to_link: Vec<_> = if all {
        installed.values().collect()
    } else if packages.is_empty() {
        return Err(WaxError::InvalidInput(
            "Specify package names or use --all to relink everything".to_string(),
        ));
    } else {
        let mut pkgs = Vec::new();
        for name in packages {
            match installed.get(name) {
                Some(pkg) => pkgs.push(pkg),
                None => {
                    eprintln!(
                        "{}: {} is not installed",
                        style("warning").yellow(),
                        style(name).magenta()
                    );
                }
            }
        }
        pkgs
    };

    if packages_to_link.is_empty() {
        return Ok(());
    }

    let mut linked = 0usize;
    let mut errors = 0usize;

    for pkg in &packages_to_link {
        let cellar = pkg.install_mode.cellar_path()?;
        let formula_dir = cellar.join(&pkg.name).join(&pkg.version);

        if !formula_dir.exists() {
            eprintln!(
                "{}: cellar directory missing for {}@{}",
                style("warning").yellow(),
                style(&pkg.name).magenta(),
                &pkg.version
            );
            errors += 1;
            continue;
        }

        match create_symlinks(
            &pkg.name,
            &pkg.version,
            &cellar,
            false,
            pkg.install_mode,
        )
        .await
        {
            Ok(links) => {
                linked += 1;
                println!(
                    "  {} {}@{} ({} links)",
                    style("✓").green(),
                    style(&pkg.name).magenta(),
                    style(&pkg.version).dim(),
                    links.len()
                );
            }
            Err(e) => {
                eprintln!(
                    "  {} {}: {}",
                    style("✗").red(),
                    style(&pkg.name).magenta(),
                    e
                );
                errors += 1;
            }
        }
    }

    println!();
    if errors > 0 {
        println!(
            "{} linked, {} errors",
            style(linked).green(),
            style(errors).red()
        );
    } else {
        println!("{} packages linked", style(linked).green());
    }

    Ok(())
}
