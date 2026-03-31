/// Nix-inspired system package management for wax.
///
/// Key design principles borrowed from Nix:
///
/// 1. **Atomic generations** — every mutating operation (install, remove, upgrade)
///    first snapshots the current installed set into a numbered, immutable
///    generation manifest.  The active generation is tracked via a `current`
///    symlink.  Any generation can be re-activated instantly.
///
/// 2. **Declarative desired state** — users declare the set of system packages
///    they want (`wax system add`).  `wax system sync` converges the live
///    system to match that declared set, adding missing and removing extraneous
///    packages.
///
/// 3. **Full rollback** — `wax system rollback [N]` reverts to any previous
///    generation by computing the diff and driving the native package manager
///    to install/remove the delta.
///
/// The actual package installation is delegated to the native OS package
/// manager (apt/dnf/pacman/apk) via `system_pm.rs`, keeping privilege
/// escalation minimal and correct.  The generation + state layer lives in
/// `~/.wax/system/`.
pub mod distro;
pub mod generations;
pub mod query;
pub mod state;

use crate::error::{Result, WaxError};
use crate::system::distro::DistroInfo;
use crate::system::generations::{Generation, GenerationManager};
use crate::system::query::list_installed;
use crate::system::state::SystemState;
use crate::system_pm::SystemPm;
use console::style;

pub struct SystemManager {
    pm: SystemPm,
    distro: DistroInfo,
    gen_mgr: GenerationManager,
}

impl SystemManager {
    /// Detect the running Linux distro and system PM.  Returns `None` on macOS
    /// or when no supported PM is found.
    pub async fn detect() -> Result<Option<Self>> {
        if cfg!(target_os = "macos") {
            return Ok(None);
        }

        let Some(distro) = DistroInfo::detect().await? else {
            return Ok(None);
        };

        let Some(pm) = SystemPm::detect().await else {
            return Ok(None);
        };

        let gen_mgr = GenerationManager::new().await?;

        Ok(Some(Self { pm, distro, gen_mgr }))
    }

    /// Name of the detected distro (e.g. "Ubuntu 22.04").
    pub fn distro_label(&self) -> String {
        if self.distro.version.is_empty() {
            self.distro.name.clone()
        } else {
            format!("{} {}", self.distro.name, self.distro.version)
        }
    }

    /// Snapshot current live packages into a new generation, then upgrade all.
    pub async fn upgrade_all(&self) -> Result<()> {
        println!(
            "{} upgrading {} packages via {}",
            style("→").cyan(),
            style(&self.distro_label()).dim(),
            style(self.pm.name()).bold()
        );

        // Snapshot *before* the upgrade so we can diff forward/back.
        self.snapshot("pre-upgrade").await?;

        self.pm.upgrade_all().await?;

        // Snapshot *after* so rollback undoes the upgrade.
        let pkgs = self.live_packages().await?;
        let after = self
            .gen_mgr
            .create("upgrade", pkgs)
            .await?;

        println!(
            "  {} generation {} created",
            style("✓").green(),
            style(after.id).bold()
        );
        Ok(())
    }

    /// Install packages, creating a new generation around the operation.
    pub async fn install(&self, packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        self.snapshot(&format!("pre-install {}", packages.join(" "))).await?;

        // Record the packages as declared and mark them installed.
        let mut st = SystemState::load().await?;
        for pkg in packages {
            st.declare(pkg);
        }

        self.pm.install(packages).await?;

        // Refresh the live list so the generation reflects reality.
        let live = self.live_packages().await?;
        for pkg in packages {
            if !st.installed.contains_key(pkg.as_str()) {
                st.mark_installed(pkg, None, true);
            }
        }
        st.save().await?;

        let gen = self.gen_mgr.create(&format!("install {}", packages.join(" ")), live).await?;

        println!(
            "  {} generation {} created",
            style("✓").green(),
            style(gen.id).bold()
        );
        Ok(())
    }

    /// Remove packages, creating a new generation around the operation.
    pub async fn remove(&self, packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        self.snapshot(&format!("pre-remove {}", packages.join(" "))).await?;

        let mut st = SystemState::load().await?;
        for pkg in packages {
            st.undeclare(pkg);
        }

        // Shell out to the native PM for actual removal.
        self.run_remove(packages).await?;

        for pkg in packages {
            st.mark_removed(pkg);
        }
        st.save().await?;

        let live = self.live_packages().await?;
        let gen = self
            .gen_mgr
            .create(&format!("remove {}", packages.join(" ")), live)
            .await?;

        println!(
            "  {} generation {} created",
            style("✓").green(),
            style(gen.id).bold()
        );
        Ok(())
    }

    /// Converge the live system to the declared package set.
    pub async fn sync_declared(&self) -> Result<()> {
        let st = SystemState::load().await?;
        if st.declared.is_empty() {
            println!("no declared system packages");
            return Ok(());
        }

        let live_set: std::collections::HashSet<_> =
            st.installed.keys().map(|s| s.as_str()).collect();
        let declared_set: std::collections::HashSet<_> =
            st.declared.iter().map(|s| s.as_str()).collect();

        let to_install: Vec<String> = declared_set
            .difference(&live_set)
            .map(|s| s.to_string())
            .collect();

        if to_install.is_empty() {
            println!("{} all declared system packages are installed", style("✓").green());
            return Ok(());
        }

        println!("installing {} missing declared packages:", to_install.len());
        for pkg in &to_install {
            println!("  {} {}", style("+").green(), style(pkg).magenta());
        }

        self.install(&to_install).await
    }

    /// List all generations.
    pub async fn list_generations(&self) -> Result<Vec<Generation>> {
        self.gen_mgr.list().await
    }

    /// Roll back to generation `id`, or to the previous generation if `None`.
    pub async fn rollback(&self, id: Option<u32>) -> Result<()> {
        let target_id = match id {
            Some(i) => i,
            None => self
                .gen_mgr
                .previous_id()
                .await?
                .ok_or_else(|| WaxError::InstallError("no previous generation to roll back to".into()))?,
        };

        let target = self
            .gen_mgr
            .get(target_id)
            .await?
            .ok_or_else(|| WaxError::InstallError(format!("generation {} not found", target_id)))?;

        let current = self.gen_mgr.current().await?;
        let current_pkgs = current.as_ref().map(|g| g.packages.as_slice()).unwrap_or(&[]);

        let (to_install, to_remove) = GenerationManager::diff(current_pkgs, &target.packages);

        if to_install.is_empty() && to_remove.is_empty() {
            println!(
                "{} already at generation {} — nothing to do",
                style("✓").green(),
                target_id
            );
            return Ok(());
        }

        println!(
            "{} rolling back to generation {} ({})",
            style("→").cyan(),
            style(target_id).bold(),
            style(&target.reason).dim()
        );

        if !to_remove.is_empty() {
            let names: Vec<String> = to_remove.iter().map(|p| p.name.clone()).collect();
            println!("  removing: {}", names.join(", "));
            self.run_remove(&names).await?;
        }

        if !to_install.is_empty() {
            let names: Vec<String> = to_install.iter().map(|p| p.name.clone()).collect();
            println!("  installing: {}", names.join(", "));
            self.pm.install(&names).await?;
        }

        // Record the rollback as its own generation so the history is complete.
        let live = self.live_packages().await?;
        let new_gen = self
            .gen_mgr
            .create(&format!("rollback to gen-{}", target_id), live)
            .await?;

        println!(
            "{} rolled back — new generation {}",
            style("✓").green(),
            style(new_gen.id).bold()
        );
        Ok(())
    }

    /// Print a status summary.
    pub async fn status(&self) -> Result<()> {
        let st = SystemState::load().await?;
        let current = self.gen_mgr.current().await?;

        println!(
            "{} {}",
            style("distro").bold(),
            style(&self.distro_label()).cyan()
        );
        println!(
            "{} {}",
            style("pm    ").bold(),
            style(self.pm.name()).cyan()
        );

        if let Some(gen) = &current {
            println!(
                "{} gen-{} ({}, {})",
                style("gen   ").bold(),
                style(gen.id).bold(),
                style(&gen.reason).dim(),
                gen.age_string()
            );
        } else {
            println!("{} none", style("gen   ").bold());
        }

        println!(
            "{} {} declared, {} installed",
            style("pkgs  ").bold(),
            st.declared.len(),
            st.installed.len()
        );

        if !st.declared.is_empty() {
            println!();
            println!("{}:", style("declared").bold());
            let live: std::collections::HashSet<_> = st.installed.keys().collect();
            for pkg in &st.declared {
                if live.contains(pkg) {
                    println!(
                        "  {} {}",
                        style("✓").green(),
                        style(pkg).magenta()
                    );
                } else {
                    println!(
                        "  {} {} {}",
                        style("✗").red(),
                        style(pkg).magenta(),
                        style("(not installed)").dim()
                    );
                }
            }
        }

        Ok(())
    }

    // ── internals ──────────────────────────────────────────────────────────

    /// Query live installed packages from the native PM.
    async fn live_packages(&self) -> Result<Vec<(String, Option<String>)>> {
        list_installed(&self.distro.format).await
    }

    /// Create a pre-op snapshot if there are any live packages to record.
    async fn snapshot(&self, reason: &str) -> Result<()> {
        let pkgs = self.live_packages().await?;
        if !pkgs.is_empty() {
            self.gen_mgr.create(reason, pkgs).await?;
        }
        Ok(())
    }

    /// Remove packages via the native PM.
    async fn run_remove(&self, packages: &[String]) -> Result<()> {
        use crate::system_pm;
        let args: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        match &self.pm {
            SystemPm::Apt => {
                let mut a = vec!["apt-get", "remove", "-y"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Dnf => {
                let mut a = vec!["dnf", "remove", "-y"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Pacman => {
                let mut a = vec!["pacman", "-R", "--noconfirm"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Apk => {
                let mut a = vec!["apk", "del"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Zypper => {
                let mut a = vec!["zypper", "remove", "-y"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Emerge => {
                let mut a = vec!["emerge", "--unmerge"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Yum => {
                let mut a = vec!["yum", "remove", "-y"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Xbps => {
                let mut a = vec!["xbps-remove", "-R"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("sudo", &a).await
            }
            SystemPm::Nix => {
                let mut a = vec!["-e"];
                a.extend_from_slice(&args);
                system_pm::run_visible_pub("nix-env", &a).await
            }
        }
    }
}
