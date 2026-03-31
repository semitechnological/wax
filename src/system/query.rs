/// Query the live system for installed package versions.
///
/// These functions shell out to read-only query tools (dpkg-query, rpm, pacman, apk)
/// to get a ground-truth snapshot of what is actually installed.  They do NOT
/// install or remove anything.
use crate::error::Result;
use crate::system::distro::PackageFormat;
use tokio::process::Command;

/// Returns a sorted list of `(name, version)` tuples for all packages the
/// system package manager currently considers installed.
pub async fn list_installed(format: &PackageFormat) -> Result<Vec<(String, Option<String>)>> {
    match format {
        PackageFormat::Deb => query_dpkg().await,
        PackageFormat::Rpm => query_rpm().await,
        PackageFormat::Pacman => query_pacman().await,
        PackageFormat::Apk => query_apk().await,
        PackageFormat::Other => Ok(vec![]),
    }
}

async fn query_dpkg() -> Result<Vec<(String, Option<String>)>> {
    let out = Command::new("dpkg-query")
        .args(["-W", r#"-f=${Package}\t${Version}\n"#])
        .output()
        .await;

    let Ok(out) = out else {
        return Ok(vec![]);
    };

    let mut pkgs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, '\t');
        let name = parts.next().unwrap_or("").trim().to_string();
        let version = parts.next().map(|v| v.trim().to_string());
        if !name.is_empty() {
            pkgs.push((name, version));
        }
    }
    pkgs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pkgs)
}

async fn query_rpm() -> Result<Vec<(String, Option<String>)>> {
    let out = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\\t%{VERSION}-%{RELEASE}\\n"])
        .output()
        .await;

    let Ok(out) = out else {
        return Ok(vec![]);
    };

    let mut pkgs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, '\t');
        let name = parts.next().unwrap_or("").trim().to_string();
        let version = parts.next().map(|v| v.trim().to_string());
        if !name.is_empty() {
            pkgs.push((name, version));
        }
    }
    pkgs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pkgs)
}

async fn query_pacman() -> Result<Vec<(String, Option<String>)>> {
    let out = Command::new("pacman")
        .args(["-Q"])
        .output()
        .await;

    let Ok(out) = out else {
        return Ok(vec![]);
    };

    let mut pkgs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(2, ' ');
        let name = parts.next().unwrap_or("").trim().to_string();
        let version = parts.next().map(|v| v.trim().to_string());
        if !name.is_empty() {
            pkgs.push((name, version));
        }
    }
    pkgs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pkgs)
}

async fn query_apk() -> Result<Vec<(String, Option<String>)>> {
    let out = Command::new("apk")
        .args(["info", "-v"])
        .output()
        .await;

    let Ok(out) = out else {
        return Ok(vec![]);
    };

    let mut pkgs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        // Format: name-version
        // Split on last '-' followed by a digit
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // apk info -v outputs "pkgname-version" where version starts with a digit
        if let Some(idx) = line.rfind('-') {
            let (name, ver) = line.split_at(idx);
            let ver = &ver[1..]; // strip leading '-'
            if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                pkgs.push((name.to_string(), Some(ver.to_string())));
                continue;
            }
        }
        pkgs.push((line.to_string(), None));
    }
    pkgs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pkgs)
}
