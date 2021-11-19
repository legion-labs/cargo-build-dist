use std::{fmt::Display, path::PathBuf};

use serde::Deserialize;

use crate::{dist_target::DistTarget, Dependencies};

use super::DockerMetadata;

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub metadata: DockerMetadata,
    pub dependencies: Dependencies,
    pub target_dir: TargetDir,
    pub package: cargo_metadata::Package,
}

impl Display for DockerPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Docker({} {})", self.package.name, self.package.version)
    }
}

impl DistTarget for DockerPackage {
    fn package(&self) -> &cargo_metadata::Package {
        &self.package
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetDir {
    pub binary_dir: PathBuf,
    pub docker_dir: PathBuf,
}
