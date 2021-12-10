//! Metadata structures for the various targets.

use std::{
    collections::BTreeMap,
    fmt::Display,
    path::{Path, PathBuf},
};

use log::debug;
use serde::{Deserialize, Deserializer};

use crate::{
    aws_lambda::AwsLambdaMetadata, dist_target::DistTarget, docker::DockerMetadata, Error,
    ErrorContext, Package, Result,
};

/// The root metadata structure.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Metadata {
    #[serde(flatten)]
    pub dist_targets: BTreeMap<String, DistTargetMetadata>,
}

impl Metadata {
    pub(crate) fn new(package_metadata: &guppy::graph::PackageMetadata<'_>) -> Result<Self> {
        #[derive(Debug, Deserialize)]
        struct RootMetadata {
            #[serde(default)]
            monorepo: Metadata,
        }

        let metadata: Option<RootMetadata> =
            serde_json::from_value(package_metadata.metadata_table().clone()).map_err(|err| {
                Error::new("failed to parse metadata")
                    .with_source(err)
                    .with_explanation(format!(
                        "failed to parse the Cargo metadata for package {}",
                        package_metadata.id()
                    ))
            })?;

        Ok(metadata
            .map(|metadata| metadata.monorepo)
            .unwrap_or_default())
    }

    pub(crate) fn dist_targets<'g>(&self, package: &'g Package<'g>) -> Vec<DistTarget<'g>> {
        self.dist_targets
            .iter()
            .map(|(name, dist_target_metadata)| {
                dist_target_metadata.to_dist_target(name.clone(), package)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum DistTargetMetadata {
    Docker(DockerMetadata),
    AwsLambda(AwsLambdaMetadata),
}

impl DistTargetMetadata {
    pub(crate) fn to_dist_target<'g>(
        &self,
        name: String,
        package: &'g Package<'g>,
    ) -> DistTarget<'g> {
        match self {
            DistTargetMetadata::Docker(docker) => docker.clone().into_dist_target(name, package),
            DistTargetMetadata::AwsLambda(lambda) => lambda.clone().into_dist_target(name, package),
        }
    }
}

impl<'de> Deserialize<'de> for DistTargetMetadata {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        enum TargetType {
            #[serde(rename = "docker")]
            Docker,
            #[serde(rename = "aws-lambda")]
            AwsLambda,
        }

        #[derive(Deserialize)]
        struct TargetHelper {
            #[serde(rename = "type")]
            target_type: TargetType,
            #[serde(flatten)]
            data: serde_value::Value,
        }

        let helper = TargetHelper::deserialize(deserializer)?;
        match helper.target_type {
            TargetType::Docker => DockerMetadata::deserialize(helper.data)
                .map(DistTargetMetadata::Docker)
                .map_err(serde::de::Error::custom),
            TargetType::AwsLambda => AwsLambdaMetadata::deserialize(helper.data)
                .map(DistTargetMetadata::AwsLambda)
                .map_err(serde::de::Error::custom),
        }
    }
}

/// A copy command instruction.
///
/// `source` indicate the files or folders to copy, possibly using glob patterns.
/// `destination` indicates the destination of the copy operation.
///
/// If `source` is a relative path, it is relative to the current package root.
/// If `destination` is always made relative to the target root.
///
/// A copy never renames files.
#[derive(Debug, Clone, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct CopyCommand {
    pub source: PathBuf,
    pub destination: PathBuf,
}

impl CopyCommand {
    pub fn source_files(&self, package_root: &Path) -> crate::Result<Vec<PathBuf>> {
        let source = if self.source.is_relative() {
            package_root.join(&self.source).display().to_string()
        } else {
            self.source.display().to_string()
        };

        let sources = glob::glob(&source)
        .map_err(|err|
            Error::new("failed to read glob pattern")
            .with_source(err)
            .with_explanation("The specified source pattern in the copy-command could not be parsed. You may want to double-check for syntax errors.")
            .with_output(format!("Copy command: {}", self))
        )?;

        sources
            .map(|entry| entry
                .map_err(|err|
                    Error::new("failed to resolve glob entry")
                    .with_source(err)
                    .with_explanation("The glob entry could not be resolved. This could be the result of a syntax error."))
                )
            .collect()
    }

    pub fn destination(&self, target_root: &Path) -> PathBuf {
        let destination = self
            .destination
            .strip_prefix("/")
            .unwrap_or(&self.destination);

        target_root.join(destination)
    }

    pub fn copy_files(&self, source_root: &Path, target_root: &Path) -> crate::Result<()> {
        let source_files = self.source_files(source_root)?;

        if source_files.is_empty() {
            debug!("No files to copy for `{}`. Moving on.", self);
            return Ok(());
        }

        let destination = self.destination(target_root);

        debug!(
            "Copying {} file(s) to to `{}`",
            source_files.len(),
            destination.display()
        );

        std::fs::create_dir_all(&destination)
            .map_err(Error::from_source)
            .with_full_context(
            "could not create target directory in Docker root",
            format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", &destination.display()),
            )?;

        let options = fs_extra::dir::CopyOptions {
            overwrite: true,
            ..fs_extra::dir::CopyOptions::default()
        };

        fs_extra::copy_items(&source_files, &destination, &options)
            .map_err(|err| Error::new("failed to copy file or directory").with_source(err))?;

        Ok(())
    }
}

impl Display for CopyCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "copy '{}' -> '{}'",
            self.source.display(),
            self.destination.display()
        )
    }
}
