use crate::cache::Cache;
use crate::commands::{install, uninstall};
use crate::error::{Result, WaxError};
use crate::install::{InstallMode, InstallState};
use console::style;

pub async fn reinstall(cache: &Cache, packages: &[String], cask: bool, all: bool) -> Result<()> {
    let state = InstallState::new()?;
    state.sync_from_cellar().await.ok();
    let installed = state.load().await?;

    let resolved: Vec<String> = if all {
        let mut names: Vec<String> = installed.keys().cloned().collect();
        names.sort();
        names
    } else {
        if packages.is_empty() {
            return Err(WaxError::InvalidInput(
                "Specify package name(s) or use --all to reinstall everything".to_string(),
            ));
        }
        packages.to_vec()
    };

    for name in &resolved {
        let install_mode = installed.get(name.as_str()).map(|p| p.install_mode);

        let (user_flag, global_flag) = match install_mode {
            Some(InstallMode::User) => (true, false),
            Some(InstallMode::Global) => (false, true),
            None => (false, false),
        };

        println!("reinstalling {}", style(name).magenta());

        if installed.contains_key(name.as_str()) {
            uninstall::uninstall_quiet(cache, name, cask).await?;
        }

        install::install_quiet(cache, std::slice::from_ref(name), cask, user_flag, global_flag)
            .await?;

        println!(
            "{} {} reinstalled",
            style("✓").green(),
            style(name).magenta()
        );
    }

    Ok(())
}
