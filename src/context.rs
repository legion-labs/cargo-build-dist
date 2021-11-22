//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands.

use cargo_metadata::PackageId;
use log::debug;
use std::{cmp::Ordering, collections::BTreeSet, fmt::Display, path::PathBuf};

use crate::{
    action_step,
    dist_target::{BuildOptions, DistTarget},
    ignore_step,
    metadata::Metadata,
    Error, Result,
};

pub enum Mode {
    Debug,
    Release,
}

impl Mode {
    pub fn from_release_flag(release_flag: bool) -> Self {
        if release_flag {
            Mode::Release
        } else {
            Mode::Debug
        }
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

#[derive(Default)]
pub struct ContextBuilder {
    manifest_path: Option<PathBuf>,
    mode: Mode,
}

impl ContextBuilder {
    pub fn build(&self) -> Result<Context> {
        debug!("Building context.");

        let metadata = self.get_metadata()?;
        let target_dir = self.get_target_dir(&metadata);

        debug!("Using target directory: {}", target_dir.display());

        let packages = self.scan_packages(&metadata)?;
        let dist_targets = self.resolve_dist_targets(metadata, &target_dir, packages)?;

        Ok(Context::new(dist_targets))
    }

    pub fn with_manifest_path(mut self, manifest_path: impl Into<PathBuf>) -> Self {
        self.manifest_path = Some(manifest_path.into());

        self
    }

    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;

        self
    }

    fn get_dependencies(
        &self,
        metadata: &cargo_metadata::Metadata,
        package_id: &PackageId,
    ) -> Result<Dependencies> {
        let resolve = match &metadata.resolve {
            Some(resolve) => resolve,
            None => {
                return Err(Error::new("`resolve` section not found in the workspace")
                    .with_explanation(format!(
                        "The `resolve` section is missing for workspace {}\
                        which prevents the resolution of dependencies.",
                        metadata.workspace_root
                    )))
            }
        };

        Ok(self
            .get_dependencies_from_resolve(resolve, package_id)?
            .map(|package_id| {
                let package = &metadata[&package_id];
                Dependency {
                    name: package.name.clone(),
                    version: package.version.to_string(),
                }
            })
            .collect())
    }

    fn get_dependencies_from_resolve<'a>(
        &self,
        resolve: &'a cargo_metadata::Resolve,
        package_id: &'a PackageId,
    ) -> Result<impl Iterator<Item = &'a PackageId>> {
        let node = resolve
            .nodes
            .iter()
            .find(|node| node.id == *package_id)
            .ok_or_else(|| {
                Error::new("could not resolve dependencies").with_explanation(format!(
                    "Unable to resolve dependencies for package {}.",
                    package_id
                ))
            })?;

        let deps: Result<Vec<&PackageId>> = node
            .dependencies
            .iter()
            .map(
                |package_id| match self.get_dependencies_from_resolve(resolve, package_id) {
                    Ok(deps) => Ok(deps),
                    Err(err) => Err(Error::new("transitive dependency failure").with_source(err)),
                },
            )
            .flat_map(|v| match v {
                Ok(v) => v.map(Ok).collect(),
                Err(e) => vec![Err(e)],
            })
            .collect();

        Ok(std::iter::once(package_id).chain(deps?.into_iter()))
    }

    fn resolve_dist_targets(
        &self,
        metadata: cargo_metadata::Metadata,
        target_dir: &PathBuf,
        packages: impl IntoIterator<Item = (PackageId, Metadata)>,
    ) -> Result<Vec<Box<dyn DistTarget>>> {
        packages
            .into_iter()
            .map(|(package_id, package_metadata)| {
                let package = &metadata[&package_id];

                debug!("Resolving package {} {}", package.name, package.version);

                let dependencies = self.get_dependencies(&metadata, &package.id)?;

                match dependencies.len() {
                    0 => debug!("Package has no dependencies"),
                    1 => debug!("Package has one dependency"),
                    x => debug!(
                        "Package has {} dependencies: {}",
                        x,
                        dependencies
                            .iter()
                            .map(Dependency::to_string)
                            .collect::<Vec<String>>()
                            .join(", "),
                    ),
                };

                let mut dist_targets: Vec<Box<dyn DistTarget>> = vec![];

                if let Some(docker) = package_metadata.docker {
                    if cfg!(windows) {
                        ignore_step!(
                            "Ignoring",
                            "distribution target `Docker` in package `{} {}` as it is not supported on Windows.",
                            package.name,
                            package.version,
                        );
                    } else {
                        dist_targets.push(Box::new(docker.into_dist_target(
                            &target_dir,
                            &package,
                            dependencies,
                        )?));
                    }
                }

                Ok(dist_targets)
            })
            .flat_map(|v| match v {
                Ok(v) => v.into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            })
            .collect()
    }

    fn scan_packages(
        &self,
        metadata: &cargo_metadata::Metadata,
    ) -> Result<Vec<(PackageId, Metadata)>> {
        metadata
            .workspace_members
            .iter()
            .filter_map(|package_id| {
                let package = &metadata[package_id];

                if package.metadata.is_null() {
                    debug!("Ignoring package without metadata: {}", package_id);

                    return None;
                }

                let metadata = match package.metadata.as_object() {
                    Some(metadata) => metadata,
                    None => {
                        return Some(Err(Error::new("package metadata is not an object")
                            .with_explanation(format!(
                    "Metadata was found for package {} but it was unexpectedly not a JSON object.",
                    package_id,
                ))));
                    }
                };

                let metadata = match metadata.get("build-dist") {
                    Some(metadata) => metadata,
                    None => {
                        debug!(
                            "Ignoring package without `build-dist` metadata: {}",
                            package_id
                        );

                        return None;
                    }
                };

                debug!("Considering package {} {}", package.name, package.version);

                let metadata = match serde_path_to_error::deserialize(metadata) {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        return Some(Err(Error::new("failed to parse `build-dist` metadata")
                            .with_source(e)
                            .with_explanation(format!(
                                "The metadata for package {} does not seem to be valid.",
                                package_id
                            ))));
                    }
                };

                Some(Ok((package_id.clone(), metadata)))
            })
            .collect()
    }

    fn get_target_dir(&self, metadata: &cargo_metadata::Metadata) -> PathBuf {
        let target_dir = PathBuf::from(
            metadata
                .target_directory
                .as_path()
                .join(self.mode.to_string()),
        );

        target_dir
    }

    fn get_metadata(&self) -> Result<cargo_metadata::Metadata> {
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
                    .with_explanation("The `CARGO` environment variable was not set: it is usually set by `cargo` itself.\nMake sure that `cargo build-dist` is run through `cargo` by putting its containing folder in your `PATH`."),
                )
            }
        }
    }
}
pub struct Context {
    dist_targets: Vec<Box<dyn DistTarget>>,
}

impl Context {
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    fn new(dist_targets: Vec<Box<dyn DistTarget>>) -> Self {
        match dist_targets.len() {
            0 => debug!("Context built successfully but has no distribution targets"),
            1 => debug!(
                "Context built successfully with one distribution target: {}",
                dist_targets[0],
            ),
            x => debug!(
                "Context built successfully with {} distribution targets: {}",
                x,
                dist_targets
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join(", "),
            ),
        };

        Self { dist_targets }
    }

    pub fn build(&self, options: BuildOptions) -> Result<()> {
        match self.dist_targets.len() {
            0 => {}
            1 => action_step!("Processing", "one distribution target",),
            x => action_step!("Processing", "{} distribution targets", x),
        };

        for dist_target in &self.dist_targets {
            action_step!("Building", dist_target.to_string());
            dist_target.build(&options)?;
        }

        Ok(())
    }
}

pub type Dependencies = BTreeSet<Dependency>;

#[derive(Debug, Eq, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

impl Display for Dependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.version)
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then(self.version.cmp(&other.version))
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Dependency {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version
    }
}
