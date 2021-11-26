use std::path::PathBuf;

use log::debug;
use serde::Deserialize;

use crate::{docker::DockerPackage, metadata::CopyCommand, Error, ErrorContext, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerMetadata {
    pub registry: String,
    pub template: String,
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

impl DockerMetadata {
    pub fn into_dist_target(
        self,
        name: String,
        target_dir: &PathBuf,
        package: &cargo_metadata::Package,
    ) -> Result<DockerPackage> {
        debug!("Package has a Docker target distribution.");

        let docker_root = target_dir.join("docker").join(&package.name);

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
            name: name,
            version: package.version.to_string(),
            toml_path: package.manifest_path.clone().into(),
            binaries,
            metadata: self,
            target_dir: target_dir.clone(),
            docker_root,
            package: package.clone(),
        })
    }
}
