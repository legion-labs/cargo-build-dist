use std::{cmp::Ordering, collections::BTreeSet, path::PathBuf};

use serde::Deserialize;

use crate::dist_target::DistTarget;

use super::DockerMetadata;

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub metadata: DockerMetadata,
    pub dependencies: BTreeSet<Dependency>,
    pub target_dir: TargetDir,
}

impl DistTarget for DockerPackage {
    fn package(&self) -> &cargo_metadata::Package {
        todo!()
    }
}

#[derive(Debug, Eq, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then(self.version.cmp(&other.version))
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Dependency {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetDir {
    pub binary_dir: PathBuf,
    pub docker_dir: PathBuf,
}
