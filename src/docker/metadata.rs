use std::path::{Path, PathBuf};

use log::debug;
use serde::Deserialize;

use crate::{metadata::CopyCommand, Error, ErrorContext, Result};

use super::DockerPackage;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerMetadata {
    #[serde(deserialize_with = "deserialize_registry")]
    pub registry: String,
    #[serde(default = "default_target_runtime")]
    pub target_runtime: String,
    pub template: String,
    #[serde(default)]
    pub extra_files: Vec<CopyCommand>,
    #[serde(default)]
    pub allow_aws_ecr_creation: bool,
    #[serde(default = "default_target_bin_dir")]
    pub target_bin_dir: PathBuf,
}

pub const DEFAULT_DOCKER_REGISTRY_ENV_VAR_NAME: &str = "CARGO_BUILD_DIST_DOCKER_REGISTRY";

fn deserialize_registry<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match String::deserialize(deserializer) {
        Ok(registry) => Ok(registry),
        Err(err) => {
            if let Ok(registry) = std::env::var(DEFAULT_DOCKER_REGISTRY_ENV_VAR_NAME) {
                Ok(registry)
            } else {
                Err(err)
            }
        }
    }
}

fn default_target_bin_dir() -> PathBuf {
    PathBuf::from("/bin")
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-gnu".to_string()
}

impl DockerMetadata {
    pub fn into_dist_target(
        self,
        name: String,
        target_root: &Path,
        package: &cargo_metadata::Package,
    ) -> Result<DockerPackage> {
        debug!("Package has a Docker target distribution.");

        let binaries: Vec<_> = package
            .targets
            .iter()
            .filter_map(|target| {
                if target.kind.contains(&"bin".to_string()) {
                    Some(target.name.clone())
                } else {
                    None
                }
            })
            .collect();

        if binaries.is_empty() {
            return Err(Error::new("package contain no binaries").with_explanation(format!("Building a Docker image requires at least one binary but the package {} does not contain any.", package.id)));
        }

        debug!(
            "Package contains the following binaries: {}",
            binaries.join(", ")
        );

        // Make sure the template is valid as soon as possible.
        tera::Template::new("", None, &self.template)
            .map_err(Error::from_source).with_full_context(
                "failed to render Dockerfile template",
                "The specified Dockerfile template could not be parsed, which may indicate a possible syntax error."
            )?;

        Ok(DockerPackage {
            name,
            version: package.version.to_string(),
            toml_path: package.manifest_path.clone().into(),
            metadata: self,
            target_root: target_root.to_path_buf(),
            package: package.clone(),
        })
    }
}
