/// Extract an .rpm package to dest_dir.
///
/// Fast path: uses `rpm2cpio` + `cpio` if available.
/// TODO: implement pure-Rust RPM header + cpio parsing as fallback.
use crate::error::{Result, WaxError};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extract an RPM and return (files, dirs). RPM tracked removal is not yet
/// supported, so empty vecs are returned.
pub fn extract_tracked(path: &Path, dest_dir: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    extract(path, dest_dir)?;
    Ok((vec![], vec![]))
}

pub fn extract(path: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;

    // Check if rpm2cpio is available
    if which_cmd("rpm2cpio") && which_cmd("cpio") {
        return extract_with_rpm2cpio(path, dest_dir);
    }

    Err(WaxError::InstallError(
        "RPM extraction requires rpm2cpio and cpio to be installed. \
         Pure-Rust RPM parsing is not yet implemented."
            .to_string(),
    ))
}

fn which_cmd(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn extract_with_rpm2cpio(path: &Path, dest_dir: &Path) -> Result<()> {
    // rpm2cpio <pkg.rpm> | cpio -idmv --no-absolute-filenames -D <dest_dir>
    let rpm2cpio = Command::new("rpm2cpio")
        .arg(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| WaxError::InstallError(format!("Failed to spawn rpm2cpio: {}", e)))?;

    let cpio_stdout = rpm2cpio
        .stdout
        .ok_or_else(|| WaxError::InstallError("rpm2cpio stdout not available".to_string()))?;

    let output = Command::new("cpio")
        .args(["-idm", "--no-absolute-filenames"])
        .current_dir(dest_dir)
        .stdin(cpio_stdout)
        .output()
        .map_err(|e| WaxError::InstallError(format!("Failed to run cpio: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WaxError::InstallError(format!(
            "cpio failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}
