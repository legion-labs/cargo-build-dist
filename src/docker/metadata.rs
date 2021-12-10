use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    dist_target::DistTarget,
    metadata::{CopyCommand, Template},
    Package,
};

use super::DockerDistTarget;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerMetadata {
    pub registry: Option<String>,
    #[serde(default = "default_target_runtime")]
    pub target_runtime: String,
    pub template: Template,
    #[serde(default)]
    pub extra_files: Vec<CopyCommand>,
    #[serde(default)]
    pub allow_aws_ecr_creation: bool,
    #[serde(default = "default_target_bin_dir")]
    pub target_bin_dir: PathBuf,
}

fn default_target_bin_dir() -> PathBuf {
    PathBuf::from("/usr/local/bin")
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-gnu".to_string()
}

impl DockerMetadata {
    pub(crate) fn into_dist_target<'g>(
        self,
        name: String,
        package: &'g Package<'g>,
    ) -> DistTarget<'g> {
        DistTarget::Docker(DockerDistTarget {
            name,
            package,
            metadata: self,
        })
    }
}
