pub mod apk;
pub mod deb;
pub mod pacman;
pub mod rpm;

use crate::error::{Result, WaxError};
use std::path::{Path, PathBuf};

/// Extract a package and return (files, dirs) — absolute paths of everything extracted.
/// `dest_dir` is the install root.
pub fn extract_package_tracked(
    path: &Path,
    dest_dir: &Path,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let name = path.to_string_lossy();
    if name.ends_with(".deb") {
        deb::extract_tracked(path, dest_dir)
    } else if name.ends_with(".pkg.tar.zst")
        || name.ends_with(".pkg.tar.xz")
        || name.ends_with(".pkg.tar.gz")
    {
        pacman::extract_tracked(path, dest_dir)
    } else if name.ends_with(".apk") {
        apk::extract_tracked(path, dest_dir)
    } else if name.ends_with(".rpm") {
        rpm::extract_tracked(path, dest_dir)
    } else {
        Err(WaxError::InstallError(format!(
            "unknown package format: {}",
            name
        )))
    }
}
