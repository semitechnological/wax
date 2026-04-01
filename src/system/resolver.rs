use crate::error::Result;
use crate::system::registry::{parse_dep_name, PackageIndex, PackageMetadata};
use std::collections::{HashSet, VecDeque};
use tracing::warn;

pub struct Resolver<'a> {
    index: &'a PackageIndex,
}

impl<'a> Resolver<'a> {
    pub fn new(index: &'a PackageIndex) -> Self {
        Self { index }
    }

    /// Resolve the full install closure for the requested packages.
    /// Returns packages in topological order (dependencies before dependents).
    pub fn resolve(&self, packages: &[String]) -> Result<Vec<&'a PackageMetadata>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut result: Vec<&'a PackageMetadata> = Vec::new();

        for pkg in packages {
            let name = parse_dep_name(pkg).to_string();
            self.visit(&name, &mut visited, &mut result);
        }

        Ok(result)
    }

    fn visit(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
        result: &mut Vec<&'a PackageMetadata>,
    ) {
        if !visited.insert(name.to_string()) {
            return;
        }

        let meta = match self.index.find(name) {
            Some(m) => m,
            None => {
                warn!("Package not found in index (skipping): {}", name);
                return;
            }
        };

        // Visit all deps recursively (DFS post-order ensures deps come first)
        for dep_raw in &meta.depends {
            let dep_name = parse_dep_name(dep_raw);
            if dep_name.is_empty() {
                continue;
            }
            self.visit(dep_name, visited, result);
        }

        // Push this package after all its deps
        result.push(meta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::registry::{PackageIndex, PackageMetadata};

    fn make_pkg(name: &str, version: &str, depends: &[&str]) -> PackageMetadata {
        PackageMetadata {
            name: name.to_string(),
            version: version.to_string(),
            description: "".to_string(),
            download_url: "".to_string(),
            sha256: None,
            installed_size: 0,
            depends: depends.iter().map(|s| s.to_string()).collect(),
            provides: vec![],
        }
    }

    #[test]
    fn test_resolve_no_deps() {
        let index = PackageIndex {
            packages: vec![make_pkg("curl", "8.0.0", &[])],
        };
        let resolver = Resolver::new(&index);
        let result = resolver.resolve(&["curl".to_string()]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "curl");
    }

    #[test]
    fn test_resolve_with_deps() {
        let index = PackageIndex {
            packages: vec![
                make_pkg("curl", "8.0.0", &["libc6", "libssl3"]),
                make_pkg("libc6", "2.35", &[]),
                make_pkg("libssl3", "3.0.0", &["libc6"]),
            ],
        };
        let resolver = Resolver::new(&index);
        let result = resolver.resolve(&["curl".to_string()]).unwrap();
        let names: Vec<_> = result.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"curl"));
        assert!(names.contains(&"libc6"));
        assert!(names.contains(&"libssl3"));
        let libc_pos = names.iter().position(|&n| n == "libc6").unwrap();
        let curl_pos = names.iter().position(|&n| n == "curl").unwrap();
        assert!(libc_pos < curl_pos);
    }

    #[test]
    fn test_resolve_missing_dep_skipped() {
        let index = PackageIndex {
            packages: vec![
                make_pkg("nginx", "1.24.0", &["libpcre3", "missing-virtual-pkg"]),
                make_pkg("libpcre3", "8.45", &[]),
            ],
        };
        let resolver = Resolver::new(&index);
        let result = resolver.resolve(&["nginx".to_string()]).unwrap();
        let names: Vec<_> = result.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"nginx"));
        assert!(names.contains(&"libpcre3"));
    }

    #[test]
    fn test_resolve_deduplicates() {
        let index = PackageIndex {
            packages: vec![
                make_pkg("curl", "8.0.0", &["libc6"]),
                make_pkg("wget", "1.21.0", &["libc6"]),
                make_pkg("libc6", "2.35", &[]),
            ],
        };
        let resolver = Resolver::new(&index);
        let result = resolver
            .resolve(&["curl".to_string(), "wget".to_string()])
            .unwrap();
        let libc_count = result.iter().filter(|p| p.name == "libc6").count();
        assert_eq!(libc_count, 1);
    }
}
