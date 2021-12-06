use std::path::{Path, PathBuf};

use log::debug;
use serde::Deserialize;

use crate::{metadata::CopyCommand, Error, ErrorContext, Package, Result};

use super::DockerPackage;

#[derive(Debug, Clone, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DockerMetadata {
    pub registry: Option<String>,
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

fn default_target_bin_dir() -> PathBuf {
    PathBuf::from("/bin")
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-gnu".to_string()
}

impl DockerMetadata {
    pub fn into_dist_target<'a>(
        self,
        name: String,
        target_root: &Path,
        package: &'a Package,
    ) -> Result<DockerPackage<'a>> {
        debug!("Package has a Docker target distribution.");

        let binaries: Vec<_> = package
            .metadata_package()
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
            return Err(Error::new("package contain no binaries").with_explanation(format!("Building a Docker image requires at least one binary but the package {} does not contain any.", package.id())));
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
            metadata: self,
            target_root: target_root.to_path_buf(),
            package,
        })
    }
}
