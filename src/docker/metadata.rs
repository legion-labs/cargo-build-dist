use std::{fmt::Display, path::PathBuf};

use log::debug;
use serde::Deserialize;

use crate::{docker::DockerPackage, Error, ErrorContext, Result};

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

#[derive(Debug, Clone, Deserialize)]
pub struct CopyCommand {
    pub source: PathBuf,
    pub destination: PathBuf,
}

impl CopyCommand {
    pub fn relative_source(&self) -> Result<PathBuf> {
        let file_name = self.source.file_name().ok_or_else(|| {
            Error::new("copy command has no source file name").with_explanation(format!(
                "The source of the copy command is not a valid path: {}",
                self.source.display()
            ))
        })?;

        let destination = self
            .destination
            .strip_prefix("/")
            .unwrap_or(&self.destination);

        Ok(destination.join(file_name))
    }
}

impl Display for CopyCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "COPY '{}' '{}'",
            self.source.display(),
            self.destination.display()
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
}
