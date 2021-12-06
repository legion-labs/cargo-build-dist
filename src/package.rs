use std::{cmp::Ordering, ffi::OsStr, fmt::Display, path::Path, process::Command};

use itertools::Itertools;
use log::debug;

use crate::{
    action_step, dist_target::DistTarget, hash::HashItem, sources::Sources, Dependencies,
    DependencyResolver, Error, Hashable, Metadata, Result,
};

/// A package in the workspace.
#[derive(Debug, Clone)]
pub struct Package {
    package: cargo_metadata::Package,
    metadata: Metadata,
    dependencies: Dependencies,
    sources: Sources,
}

impl Package {
    pub(crate) fn from(
        package: &cargo_metadata::Package,
        resolver: &impl DependencyResolver,
        sources: Sources,
    ) -> Result<Self> {
        let metadata = Self::metadata_from_cargo_metadata_package(package)?;
        let dependencies = resolver.resolve(&package.id)?;

        Ok(Self {
            package: package.clone(),
            metadata,
            dependencies,
            sources,
        })
    }

    pub fn name(&self) -> &str {
        &self.package.name
    }

    pub fn version(&self) -> &cargo_metadata::Version {
        &self.package.version
    }

    pub fn id(&self) -> &cargo_metadata::PackageId {
        &self.package.id
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    pub fn execute(
        &self,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<std::process::ExitStatus> {
        let args: Vec<_> = args.into_iter().collect();

        if args.is_empty() {
            return Err(Error::new("no arguments provided to execute"));
        }

        action_step!("Executing", "{}", self.id());
        action_step!(
            "Running",
            "`{}`",
            args.iter().map(|s| s.as_ref().to_string_lossy()).join(" "),
        );

        let program = args[0].as_ref();
        let program_args = &args[1..];
        let mut cmd = Command::new(program);

        cmd.args(program_args)
            .current_dir(&self.package.manifest_path.parent().unwrap());

        cmd.status()
            .map_err(|err| Error::new("failed to execute command").with_source(err))
    }

    fn metadata_from_cargo_metadata_package(package: &cargo_metadata::Package) -> Result<Metadata> {
        if package.metadata.is_null() {
            debug!("Package has no metadata: {}", package.id);

            return Ok(Metadata::default());
        }

        let metadata = match package.metadata.as_object() {
            Some(metadata) => metadata,
            None => {
                return Err(
                    Error::new("package metadata is not an object").with_explanation(format!(
                    "Metadata was found for package {} but it was unexpectedly not a JSON object.",
                    package.id,
                )),
                );
            }
        };

        let metadata = if let Some(metadata) = metadata.get("monorepo") {
            metadata
        } else {
            debug!("Package has no monorepo metadata: {}", package.id);

            return Ok(Metadata::default());
        };

        serde_path_to_error::deserialize(metadata).map_err(|err| {
            Error::new("failed to parse monorepo metadata")
                .with_source(err)
                .with_explanation(format!(
                    "The metadata for package {} does not seem to be valid.",
                    package.id
                ))
        })
    }

    pub(crate) fn resolve_dist_targets(
        &self,
        target_root: &Path,
    ) -> Result<Vec<Box<dyn DistTarget>>> {
        let mut dist_targets: Vec<Box<dyn DistTarget>> = vec![];

        for (name, target) in self.metadata.targets.iter().sorted_unstable() {
            dist_targets.push(target.clone().into_dist_target(
                name.clone(),
                target_root,
                &self.package,
            )?);
        }

        Ok(dist_targets)
    }
}

impl Hashable for Package {
    fn as_hash_item(&self) -> crate::hash::HashItem<'_> {
        HashItem::List(vec![
            HashItem::named("dependencies", self.dependencies.as_hash_item()),
            HashItem::named("sources", self.sources.as_hash_item()),
        ])
    }
}

impl Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.package.name)
    }
}

impl Eq for Package {}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> Ordering {
        self.package.id.cmp(&other.package.id)
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.package.id == other.package.id
    }
}
