//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands.

use git2::Repository;
use log::debug;
use std::{collections::BTreeSet, path::PathBuf, time::Instant};

use crate::{action_step, sources::Sources, DistTarget, Error, Options, Package, Result};

/// Build a context from the current environment and optionally provided
/// attributes.
#[derive(Default)]
pub struct ContextBuilder {
    manifest_path: Option<PathBuf>,
}

impl ContextBuilder {
    /// Create a new `Context` using the current parameters.
    pub fn build(self) -> Result<Context> {
        debug!("Building context.");

        let manifest_path = if let Some(manifest_path) = self.manifest_path {
            manifest_path
        } else {
            let cwd = std::env::current_dir().map_err(|err| {
                Error::new("could not determine current directory").with_source(err)
            })?;

            cwd.join("Cargo.toml")
        };

        let manifest_path = std::fs::canonicalize(manifest_path)
            .map_err(|err| Error::new("could not find Cargo.toml").with_source(err))?;

        Context::new(manifest_path)
    }

    /// Specify the path to the manifest file to use.
    ///
    /// If not called, the default is to use the manifest file in the current
    /// working directory.
    pub fn with_manifest_path(mut self, manifest_path: impl Into<PathBuf>) -> Self {
        self.manifest_path = Some(manifest_path.into());

        self
    }
}
/// A build context.
pub struct Context {
    manifest_path: PathBuf,
    config: cargo::util::Config,
}

impl Context {
    /// Create a new `ContextBuilder`.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    fn new(manifest_path: PathBuf) -> Result<Self> {
        let config = cargo::util::config::Config::default()
            .map_err(|err| Error::new("failed to load Cargo configuration").with_source(err))?;

        Ok(Self {
            manifest_path,
            config,
        })
    }

    fn get_global_metadata(&self) -> Result<cargo_metadata::Metadata> {
        let mut cmd = cargo_metadata::MetadataCommand::new();

        // MetadataCommand::new() would actually perform the same logic, but we
        // want the error to be explicit if it happens.
        let cargo = Self::get_cargo_path()?;

        debug!("Using `cargo` at: {}", cargo.display());

        cmd.cargo_path(cargo);
        cmd.manifest_path(&self.manifest_path);

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

    pub fn workspace(&self) -> Result<cargo::core::Workspace<'_>> {
        cargo::core::Workspace::new(&self.manifest_path, &self.config)
            .map_err(|err| Error::new("failed to load Cargo workspace").with_source(err))
    }

    pub fn packages(&self) -> Result<BTreeSet<Package>> {
        let workspace = self.workspace()?;
        let metadata = self.get_global_metadata()?;

        metadata
            .workspace_members
            .iter()
            .map(|package_id| {
                let package = &metadata[package_id];

                let pkg = workspace
                    .members()
                    .find(|pkg| pkg.name().as_str() == package.name)
                    .ok_or_else(|| {
                        Error::new("failed to find package").with_explanation(format!(
                            "Could not find a package named `{}` in the current workspace.",
                            package_id
                        ))
                    })?;

                let sources = Sources::scan_package(pkg, &workspace)?;
                Package::from(package, &metadata, sources)
            })
            .collect()
    }

    pub fn get_package_by_name(&self, name: &'_ str) -> Result<Package> {
        self.packages()?.iter().find(|p| p.name() == name).cloned().ok_or_else(|| {
                Error::new("package not found")
                    .with_explanation(format!("The operation was attempted onto a package that was not found in the current workspace: {}", name))
        })
    }

    pub fn get_packages_by_names<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<BTreeSet<Package>> {
        names
            .into_iter()
            .map(|name| self.get_package_by_name(name))
            .collect::<Result<Vec<_>>>()
            .map(|v| v.into_iter().collect())
    }

    pub fn git_repository(&self) -> Result<Repository> {
        let workspace = self.workspace()?;

        Repository::open(workspace.root())
            .map_err(|err| Error::new("failed to open Git repository").with_source(err))
    }

    pub fn get_changed_packages(&self, start: &str) -> Result<BTreeSet<Package>> {
        let packages = self.packages()?;
        let changed_files = self.get_changed_files(start)?;

        Ok(packages
            .into_iter()
            .filter(|p| {
                for changed_file in &changed_files {
                    if p.sources().contains(changed_file) {
                        return true;
                    }
                }

                false
            })
            .collect())
    }

    fn get_changed_files(&self, start: &str) -> Result<BTreeSet<PathBuf>> {
        let repo = self.git_repository()?;
        let start = repo
            .revparse_single(start)
            .map_err(|err| Error::new("failed to parse Git revision").with_source(err))?
            .as_commit()
            .ok_or_else(|| Error::new("reference is not a commit"))?
            .tree()
            .unwrap();

        let diff = repo
            .diff_tree_to_workdir(Some(&start), None)
            .map_err(|err| Error::new("failed to generate diff").with_source(err))?;

        let prefix = repo
            .path()
            .parent()
            .ok_or_else(|| Error::new("failed to determine Git repository path"))?;

        let mut result = Vec::new();

        // 100 is as good a guess as any.
        result.reserve(100);

        diff.print(git2::DiffFormat::NameOnly, |_, _, l| {
            let path = prefix.join(PathBuf::from(
                std::str::from_utf8(l.content()).unwrap().trim_end(),
            ));

            result.push(
                std::fs::canonicalize(path)
                    .map_err(|err| Error::new("failed to get canonical path").with_source(err)),
            );

            true
        })
        .map_err(|err| Error::new("failed to print diff").with_source(err))?;

        result
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .map(|p| p.into_iter().collect())
    }

    fn get_dist_targets_for(
        &self,
        packages: &BTreeSet<Package>,
    ) -> Result<Vec<Box<dyn DistTarget>>> {
        let global_metadata = self.get_global_metadata()?;
        let target_root = PathBuf::from(global_metadata.target_directory.as_path());

        Ok(packages
            .iter()
            .map(|package| package.resolve_dist_targets(&target_root))
            .collect::<Result<Vec<Vec<Box<dyn DistTarget>>>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    /// Build all the collected distribution targets.
    pub fn build_dist_targets(
        &self,
        packages: &BTreeSet<Package>,
        options: &Options,
    ) -> Result<()> {
        let dist_targets = self.get_dist_targets_for(packages)?;

        match dist_targets.len() {
            0 => {}
            1 => action_step!("Processing", "one distribution target",),
            x => action_step!("Processing", "{} distribution targets", x),
        };

        for dist_target in &dist_targets {
            action_step!("Building", dist_target.to_string());
            let now = Instant::now();

            dist_target.build(options)?;

            action_step!(
                "Finished",
                "{} in {:.2}s",
                dist_target,
                now.elapsed().as_secs_f64()
            );
        }

        Ok(())
    }

    /// Publish all the collected distribution targets.
    pub fn publish_dist_targets(
        &self,
        packages: &BTreeSet<Package>,
        options: &Options,
    ) -> Result<()> {
        let dist_targets = self.get_dist_targets_for(packages)?;

        match dist_targets.len() {
            0 => {}
            1 => action_step!("Processing", "one distribution target",),
            x => action_step!("Processing", "{} distribution targets", x),
        };

        for dist_target in &dist_targets {
            action_step!("Publishing", dist_target.to_string());
            let now = Instant::now();

            dist_target.publish(options)?;

            action_step!(
                "Finished",
                "{} in {:.2}s",
                dist_target,
                now.elapsed().as_secs_f64()
            );
        }

        Ok(())
    }
}
