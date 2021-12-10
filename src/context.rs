//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands.

use git2::Repository;
use guppy::graph::DependencyDirection;
use itertools::Itertools;
use log::debug;
use std::{fmt::Display, path::PathBuf};

use crate::{Error, Package, Result};

#[derive(Default, Debug)]
pub struct Options {
    pub dry_run: bool,
    pub force: bool,
    pub verbose: bool,
    pub mode: Mode,
}

/// A build mode that can either be `Debug` or `Release`.
#[derive(Debug, Clone)]
pub enum Mode {
    Debug,
    Release,
}

impl Mode {
    pub fn from_release_flag(release_flag: bool) -> Self {
        if release_flag {
            Self::Release
        } else {
            Self::Debug
        }
    }

    pub fn is_debug(&self) -> bool {
        matches!(self, Self::Debug)
    }

    pub fn is_release(&self) -> bool {
        matches!(self, Self::Release)
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self::Debug
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
        }
    }
}
/// Build a context from the current environment and optionally provided
/// attributes.
#[derive(Default)]
pub struct ContextBuilder {
    manifest_path: Option<PathBuf>,
    options: Options,
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

        Context::new(manifest_path, self.options)
    }

    /// Specify the path to the manifest file to use.
    ///
    /// If not called, the default is to use the manifest file in the current
    /// working directory.
    pub fn with_manifest_path(mut self, manifest_path: impl Into<PathBuf>) -> Self {
        self.manifest_path = Some(manifest_path.into());

        self
    }

    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;

        self
    }
}
/// A build context.
#[derive(Debug)]
pub struct Context {
    manifest_path: PathBuf,
    options: Options,
    config: cargo::util::Config,
    package_graph: guppy::graph::PackageGraph,
}

impl Context {
    /// Create a new `ContextBuilder`.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    fn new(manifest_path: PathBuf, options: Options) -> Result<Self> {
        let config = cargo::util::config::Config::default()
            .map_err(|err| Error::new("failed to load Cargo configuration").with_source(err))?;

        let mut cmd = guppy::MetadataCommand::new();
        cmd.manifest_path(&manifest_path);

        let package_graph = guppy::graph::PackageGraph::from_command(&mut cmd)
            .map_err(|err| Error::new("failed to parse package graph").with_source(err))?;

        Ok(Self {
            manifest_path,
            options,
            config,
            package_graph,
        })
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn workspace(&self) -> Result<cargo::core::Workspace<'_>> {
        cargo::core::Workspace::new(&self.manifest_path, &self.config)
            .map_err(|err| Error::new("failed to load Cargo workspace").with_source(err))
    }

    pub fn target_root(&self) -> Result<PathBuf> {
        let workspace = self.workspace()?;

        Ok(workspace.target_dir().into_path_unlocked())
    }

    pub fn packages(&self) -> Result<Vec<Package<'_>>> {
        self.package_graph
            .packages()
            .filter_map(|package_metadata| {
                if package_metadata.source().is_workspace() {
                    Some(Package::new(self, package_metadata))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>>>()
            .map(|packages| {
                packages
                    .into_iter()
                    .sorted_by(|a, b| a.name().cmp(b.name()))
                    .collect()
            })
    }

    pub fn resolve_package_by_name(&self, name: &str) -> Result<Package<'_>> {
        let package_set = self.package_graph.resolve_package_name(name);

        if package_set.is_empty() {
            return Err(Error::new("package not found").with_explanation(format!(
                "A cargo package with the given name ({}) could not be found.",
                name
            )));
        }

        let package_metadata = package_set
            .packages(DependencyDirection::Forward)
            .next()
            .unwrap();

        Package::new(self, package_metadata)
    }

    pub fn resolve_packages_by_names<'b>(
        &self,
        names: impl IntoIterator<Item = &'b str>,
    ) -> Result<Vec<Package<'_>>> {
        names
            .into_iter()
            .map(|name| self.resolve_package_by_name(name))
            .collect()
    }

    pub fn resolve_changed_packages(&self, start: &str) -> Result<Vec<Package<'_>>> {
        let changed_files = self.get_changed_files(start)?;

        Ok(self
            .packages()?
            .into_iter()
            .filter_map(|p| {
                for changed_file in &changed_files {
                    if p.sources().contains(changed_file) {
                        return Some(
                            p.dependant_packages()
                                .map(|packages| std::iter::once(p).chain(packages)),
                        );
                    }
                }

                None
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    fn git_repository(&self) -> Result<Repository> {
        Repository::open(self.workspace()?.root())
            .map_err(|err| Error::new("failed to open Git repository").with_source(err))
    }

    fn get_changed_files(&self, start: &str) -> Result<Vec<PathBuf>> {
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

        diff.print(git2::DiffFormat::NameOnly, |_, _, l| {
            let path = prefix.join(PathBuf::from(
                std::str::from_utf8(l.content()).unwrap().trim_end(),
            ));

            result.push(path);

            true
        })
        .map_err(|err| Error::new("failed to print diff").with_source(err))?;

        Ok(result)
    }

    ///// Build all the collected distribution targets.
    //pub fn build_dist_targets<'a>(
    //    &self,
    //    packages: impl IntoIterator<Item = &'a Package>,
    //) -> Result<()> {
    //    let dist_targets: Vec<&DistTarget> = Self::get_dist_targets_for(packages).collect();

    //    match dist_targets.len() {
    //        0 => {}
    //        1 => action_step!("Processing", "one distribution target",),
    //        x => action_step!("Processing", "{} distribution targets", x),
    //    };

    //    for dist_target in dist_targets {
    //        action_step!("Building", dist_target.to_string());
    //        let now = Instant::now();

    //        dist_target.build(self)?;

    //        action_step!(
    //            "Finished",
    //            "{} in {:.2}s",
    //            dist_target,
    //            now.elapsed().as_secs_f64()
    //        );
    //    }

    //    Ok(())
    //}

    ///// Publish all the collected distribution targets.
    //pub fn publish_dist_targets<'a>(
    //    &self,
    //    packages: impl IntoIterator<Item = &'a Package>,
    //) -> Result<()> {
    //    let dist_targets: Vec<&DistTarget> = Self::get_dist_targets_for(packages).collect();

    //    match dist_targets.len() {
    //        0 => {}
    //        1 => action_step!("Processing", "one distribution target",),
    //        x => action_step!("Processing", "{} distribution targets", x),
    //    };

    //    for dist_target in &dist_targets {
    //        if dist_target.package().tag_matches()? {
    //            action_step!("Publishing", dist_target.to_string());
    //            let now = Instant::now();

    //            dist_target.publish(self)?;

    //            action_step!(
    //                "Finished",
    //                "{} in {:.2}s",
    //                dist_target,
    //                now.elapsed().as_secs_f64()
    //            );
    //        } else {
    //            ignore_step!(
    //                "Skipping",
    //                "{} as the current hash does not match its tag",
    //                dist_target,
    //            );
    //        }
    //    }

    //    Ok(())
    //}
}
