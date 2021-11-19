use std::path::PathBuf;

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
}

impl DistTarget for DockerPackage {
    fn package(&self) -> &cargo_metadata::Package {
        todo!()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetDir {
    pub binary_dir: PathBuf,
    pub docker_dir: PathBuf,
}
