use std::path::PathBuf;

use log::debug;
use serde::Deserialize;

use crate::{aws_lambda::AwsLambdaPackage, metadata::CopyCommand, Error, Mode, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsLambdaMetadata {
    pub s3_bucket: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub s3_bucket_prefix: String,
    #[serde(default = "default_target_runtime")]
    pub target_runtime: String,
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub extra_files: Vec<CopyCommand>,
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-musl".to_string()
}

impl AwsLambdaMetadata {
    pub fn into_dist_target(
        self,
        name: String,
        target_root: &PathBuf,
        mode: &Mode,
        package: &cargo_metadata::Package,
    ) -> Result<AwsLambdaPackage> {
        debug!("Package has an AWS Lambda target distribution.");

        let target_dir = target_root
            .join(&self.target_runtime)
            .join(mode.to_string());
        let lambda_root = target_dir.join("aws-lambda").join(&package.name);

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
            return Err(
                Error::new("package contain no binaries").with_explanation(
                    format!(
                        "Building an AWS Lambda requires at least one binary but the package {} does not contain any.",
                        package.id,
                    ),
                ),
            );
        }

        let binary = if self.binary.is_empty() {
            if binaries.len() > 1 {
                return Err(
                    Error::new("no binary specified").with_explanation(
                        format!(
                            "Building an AWS Lambda requires a single binary for the package {} but no specific one was configured and the package contains multiple binaries: {}",
                            package.id, binaries.join(", "),
                        ),
                    ),
                );
            } else {
                binaries[0].clone()
            }
        } else if !binaries.contains(&self.binary) {
            return Err(
                Error::new("package contains no binary with the specified name").with_explanation(
                    format!(
                        "The package {} does not contain a binary with the name {}.",
                        package.id, self.binary
                    ),
                ),
            );
        } else {
            self.binary.clone()
        };

        debug!("Package uses the following binary: {}", binary);

        let mode = mode.clone();

        Ok(AwsLambdaPackage {
            name: name,
            version: package.version.to_string(),
            toml_path: package.manifest_path.clone().into(),
            binary,
            metadata: self,
            target_dir: target_dir.clone(),
            lambda_root,
            mode,
            package: package.clone(),
        })
    }
}
