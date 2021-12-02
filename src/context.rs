//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands.

use log::debug;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use crate::{Error, Package, Result};

/// Build a context from the current environment and optionally provided
/// attributes.
#[derive(Default)]
pub struct ContextBuilder {
    manifest_path: Option<PathBuf>,
}

impl ContextBuilder {
    /// Create a new `Context` using the current parameters.
    pub fn build(&self) -> Result<Context> {
        debug!("Building context.");

        let metadata = self.get_global_metadata()?;
        let packages = Self::scan_packages(&metadata)?;

        Ok(Context::new(packages))
    }

    /// Specify the path to the manifest file to use.
    ///
    /// If not called, the default is to use the manifest file in the current
    /// working directory.
    pub fn with_manifest_path(mut self, manifest_path: impl Into<PathBuf>) -> Self {
        self.manifest_path = Some(manifest_path.into());

        self
    }

    fn scan_packages(metadata: &cargo_metadata::Metadata) -> Result<BTreeSet<Package>> {
        metadata
            .workspace_members
            .iter()
            .map(|package_id| {
                let package = &metadata[package_id];

                Package::from_cargo_metadata_package(package, &metadata)
            })
            .collect()
    }

    fn get_global_metadata(&self) -> Result<cargo_metadata::Metadata> {
        let mut cmd = cargo_metadata::MetadataCommand::new();

        // MetadataCommand::new() would actually perform the same logic, but we
        // want the error to be explicit if it happens.
        let cargo = Self::get_cargo_path()?;

        debug!("Using `cargo` at: {}", cargo.display());

        cmd.cargo_path(cargo);

        if let Some(manifest_path) = &self.manifest_path {
            cmd.manifest_path(manifest_path);
        }

        cmd.exec()
            .map_err(|e| Error::new("failed to query cargo metadata").with_source(e))
    }

    fn get_cargo_path() -> Result<PathBuf> {
        match std::env::var("CARGO") {
            Ok(cargo) => Ok(PathBuf::from(&cargo)),
            Err(e) => {
                Err(
                    Error::new("`cargo` not found")
                    .with_source(e)
                    .with_explanation("The `CARGO` environment variable was not set: it is usually set by `cargo` itself.\nMake sure that `cargo monorepo` is run through `cargo` by putting its containing folder in your `PATH`."),
                )
            }
        }
    }
}
/// A build context.
pub struct Context {
    packages: BTreeSet<Package>,
}

impl Context {
    /// Create a new `ContextBuilder`.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    fn new(packages: BTreeSet<Package>) -> Self {
        Self { packages }
    }

    pub fn packages(&self) -> &BTreeSet<Package> {
        &self.packages
    }

    pub fn list_packages(&self) {
        for package in self.packages() {
            println!("{}", package);
        }
    }

    ///// Build all the collected distribution targets.
    //pub fn build_dist_targets(&self, options: &BuildOptions) -> Result<()> {
    //    match self.dist_targets.len() {
    //        0 => {}
    //        1 => action_step!("Processing", "one distribution target",),
    //        x => action_step!("Processing", "{} distribution targets", x),
    //    };

    //    for dist_target in &self.dist_targets {
    //        action_step!("Building", dist_target.to_string());
    //        let now = Instant::now();

    //        match dist_target.build(options)? {
    //            BuildResult::Success => {
    //                action_step!(
    //                    "Finished",
    //                    "{} in {:.2}s",
    //                    dist_target,
    //                    now.elapsed().as_secs_f64()
    //                );
    //            }
    //            BuildResult::Ignored(reason) => {
    //                ignore_step!("Ignored", "{}", reason,);
    //            }
    //        }
    //    }

    //    Ok(())
    //}
}

fn get_target_root(global_metadata: &cargo_metadata::Metadata) -> PathBuf {
    PathBuf::from(global_metadata.target_directory.as_path())
}
