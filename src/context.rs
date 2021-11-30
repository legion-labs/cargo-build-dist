//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands.

use cargo_metadata::PackageId;
use log::debug;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::BTreeSet,
    fmt::Display,
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    action_step,
    dist_target::{BuildOptions, BuildResult, DistTarget},
    ignore_step,
    metadata::Metadata,
    Error, Result,
};

#[derive(Default)]
pub struct ContextBuilder {
    manifest_path: Option<PathBuf>,
}

impl ContextBuilder {
    pub fn build(&self) -> Result<Context> {
        debug!("Building context.");

        let metadata = self.get_metadata()?;
        let target_root = Self::get_target_root(&metadata);

        debug!("Using target directory: {}", target_root.display());

        let packages = Self::scan_packages(&metadata)?;
        let dist_targets = self.resolve_dist_targets(&metadata, &target_root, packages)?;

        Ok(Context::new(dist_targets))
    }

    pub fn with_manifest_path(mut self, manifest_path: impl Into<PathBuf>) -> Self {
        self.manifest_path = Some(manifest_path.into());

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
                let package = &metadata[package_id];
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
        metadata: &cargo_metadata::Metadata,
        target_root: &Path,
        packages: impl IntoIterator<Item = (PackageId, Metadata)>,
    ) -> Result<Vec<Box<dyn DistTarget>>> {
        packages
            .into_iter()
            .map(|(package_id, package_metadata)| {
                let package = &metadata[&package_id];

                debug!("Resolving package {} {}", package.name, package.version);

                if let Some(deps_hash) = package_metadata.deps_hash {
                    debug!("Package has a dependency hash specified: making sure it is up-to-date.");

                    let dependencies = self.get_dependencies(metadata, &package.id)?;

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

                    let current_deps_hash = get_dependencies_hash(&dependencies);

                    if current_deps_hash != deps_hash {
                        return Err(
                            Error::new("dependencies hash does not match")
                            .with_explanation("The specified dependency hash does not match the actual computed version.\n\n\
                            This may indicate that some dependencies have changed and may require a major/minor version bump. \n\n\
                            Please validate this and update the dependencies hash to confirm the new dependencies.")
                            .with_output(format!(
                                "Expected: {}\n  \
                                Actual: {}",
                                deps_hash,
                                current_deps_hash
                            ))
                        );
                    }

                    debug!("Package dependency hash is up-to-date. Moving on.");
                }

                let mut dist_targets: Vec<Box<dyn DistTarget>> = vec![];

                for (name, target) in package_metadata.targets {
                    dist_targets.push(target.into_dist_target(name.clone(), target_root, package)?);
                }

                Ok(dist_targets)
            })
            .flat_map(|v| match v {
                Ok(v) => v.into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            })
            .collect()
    }

    fn scan_packages(metadata: &cargo_metadata::Metadata) -> Result<Vec<(PackageId, Metadata)>> {
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

                let metadata = if let Some(metadata) = metadata.get("build-dist") {
                    metadata
                } else {
                    debug!(
                        "Ignoring package without `build-dist` metadata: {}",
                        package_id
                    );

                    return None;
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

    fn get_target_root(metadata: &cargo_metadata::Metadata) -> PathBuf {
        PathBuf::from(metadata.target_directory.as_path())
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
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", "),
            ),
        };

        Self { dist_targets }
    }

    pub fn build(&self, options: &BuildOptions) -> Result<()> {
        match self.dist_targets.len() {
            0 => {}
            1 => action_step!("Processing", "one distribution target",),
            x => action_step!("Processing", "{} distribution targets", x),
        };

        for dist_target in &self.dist_targets {
            action_step!("Building", dist_target.to_string());
            let now = Instant::now();

            match dist_target.build(options)? {
                BuildResult::Success => {
                    action_step!(
                        "Finished",
                        "{} in {:.2}s",
                        dist_target,
                        now.elapsed().as_secs_f64()
                    );
                }
                BuildResult::Ignored(reason) => {
                    ignore_step!("Ignored", "{}", reason,);
                }
            }
        }

        Ok(())
    }
}

type Dependencies = BTreeSet<Dependency>;

#[derive(Debug, Eq, Clone)]
struct Dependency {
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

fn get_dependencies_hash(dependencies: &Dependencies) -> String {
    let mut deps_hasher = Sha256::new();

    for dep in dependencies {
        deps_hasher.update(&dep.name);
        deps_hasher.update(" ");
        deps_hasher.update(&dep.version);
    }

    format!("{:x}", deps_hasher.finalize())
}
