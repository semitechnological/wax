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
    /// Returns packages in BFS order (dependencies before dependents).
    pub fn resolve(&self, packages: &[String]) -> Result<Vec<&'a PackageMetadata>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result: Vec<&'a PackageMetadata> = Vec::new();

        for pkg in packages {
            let name = parse_dep_name(pkg).to_string();
            if visited.insert(name.clone()) {
                queue.push_back(name);
            }
        }

        while let Some(name) = queue.pop_front() {
            let meta = match self.index.find(&name) {
                Some(m) => m,
                None => {
                    warn!("Package not found in index (skipping): {}", name);
                    continue;
                }
            };

            // Queue unvisited dependencies first so they come before this package
            let mut dep_indices = Vec::new();
            for dep_raw in &meta.depends {
                let dep_name = parse_dep_name(dep_raw).to_string();
                if dep_name.is_empty() {
                    continue;
                }
                if visited.insert(dep_name.clone()) {
                    dep_indices.push(dep_name);
                }
            }

            // Prepend deps to the front so they resolve before the current package
            for dep in dep_indices.into_iter().rev() {
                queue.push_front(dep);
            }

            result.push(meta);
        }

        Ok(result)
    }
}
