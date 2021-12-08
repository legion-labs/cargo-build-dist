use std::path::PathBuf;

use serde::Deserialize;

use crate::{dist_target::DistTarget, metadata::CopyCommand, Package};

use super::DockerDistTarget;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerMetadata {
    pub registry: Option<String>,
    #[serde(default = "default_target_runtime")]
    pub target_runtime: String,
    #[serde(deserialize_with = "deserialize_template")]
    pub template: tera::Tera,
    #[serde(default)]
    pub extra_files: Vec<CopyCommand>,
    #[serde(default)]
    pub allow_aws_ecr_creation: bool,
    #[serde(default = "default_target_bin_dir")]
    pub target_bin_dir: PathBuf,
}

fn default_target_bin_dir() -> PathBuf {
    PathBuf::from("/bin")
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-gnu".to_string()
}

fn deserialize_template<'de, D>(data: D) -> std::result::Result<tera::Tera, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(data)?;

    let mut result = tera::Tera::default();

    result
        .add_raw_template("dockerfile", &s)
        .map_err(serde::de::Error::custom)?;

    Ok(result)
}

impl DockerMetadata {
    pub(crate) fn into_dist_target<'g>(self, name: String, package: Package<'g>) -> DistTarget<'g> {
        DistTarget::Docker(DockerDistTarget {
            name,
            package,
            metadata: self,
        })
    }
}
