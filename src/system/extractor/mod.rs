pub mod apk;
pub mod deb;
pub mod pacman;
pub mod rpm;

use crate::error::{Result, WaxError};
use std::path::Path;

/// Extract a downloaded package file to `dest_dir`.
/// Dispatches based on file extension.
pub fn extract_package(path: &Path, dest_dir: &Path) -> Result<()> {
    let name = path.to_string_lossy();
    if name.ends_with(".deb") {
        deb::extract(path, dest_dir)
    } else if name.ends_with(".pkg.tar.zst")
        || name.ends_with(".pkg.tar.xz")
        || name.ends_with(".pkg.tar.gz")
    {
        pacman::extract(path, dest_dir)
    } else if name.ends_with(".apk") {
        apk::extract(path, dest_dir)
    } else if name.ends_with(".rpm") {
        rpm::extract(path, dest_dir)
    } else {
        Err(WaxError::InstallError(format!(
            "unknown package format: {}",
            name
        )))
    }
}
