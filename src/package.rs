use std::{cmp::Ordering, fmt::Display};

use log::debug;

use crate::{Dependencies, DependencyResolver, Error, Metadata, Result};

/// A package in the workspace.
#[derive(Debug, Clone)]
pub struct Package {
    package: cargo_metadata::Package,
    metadata: Metadata,
    dependencies: Dependencies,
}

impl Package {
    pub(crate) fn from_cargo_metadata_package(
        package: &cargo_metadata::Package,
        resolver: &impl DependencyResolver,
    ) -> Result<Self> {
        let metadata = Self::metadata_from_cargo_metadata_package(package)?;
        let dependencies = resolver.resolve(&package.id)?;

        Ok(Self {
            package: package.clone(),
            metadata,
            dependencies,
        })
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

    //fn resolve_dist_targets(&self, target_root: &Path) -> Result<Vec<Box<dyn DistTarget>>> {
    //    let mut dist_targets: Vec<Box<dyn DistTarget>> = vec![];

    //    for (name, target) in &self.metadata.targets {
    //        dist_targets.push(target.into_dist_target(name.clone(), target_root, package)?);
    //    }

    //    Ok(dist_targets)
    //}
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
