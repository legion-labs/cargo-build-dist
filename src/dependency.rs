use cargo_metadata::PackageId;
use std::{cmp::Ordering, collections::BTreeMap, fmt::Display};

use crate::{hash::HashItem, Error, Hashable, Result};

#[derive(Default, Debug, Clone)]
pub(crate) struct Dependencies(BTreeMap<PackageId, Dependency>);

impl Hashable for Dependencies {
    fn as_hash_item(&self) -> crate::hash::HashItem<'_> {
        self.0.values().map(Hashable::as_hash_item).collect()
    }
}

#[derive(Debug, Eq, Clone)]
pub(crate) struct Dependency {
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

impl Hashable for Dependency {
    fn as_hash_item(&self) -> crate::hash::HashItem<'_> {
        HashItem::List(vec![
            HashItem::named("name", HashItem::String(&self.name)),
            HashItem::named("version", HashItem::String(&self.version)),
        ])
    }
}

pub(crate) trait DependencyResolver {
    fn resolve(&self, package_id: &cargo_metadata::PackageId) -> Result<Dependencies> {
        let result = Dependencies::default();

        self.resolve_with(result, package_id)
    }

    fn resolve_with(
        &self,
        result: Dependencies,
        package_id: &cargo_metadata::PackageId,
    ) -> Result<Dependencies>;
}

impl DependencyResolver for cargo_metadata::Metadata {
    fn resolve_with(
        &self,
        mut result: Dependencies,
        package_id: &cargo_metadata::PackageId,
    ) -> Result<Dependencies> {
        if result.0.contains_key(package_id) {
            return Ok(result);
        }

        let dependency = {
            let package = &self[package_id];

            Dependency {
                name: package.name.clone(),
                version: package.version.to_string(),
            }
        };

        result.0.insert(package_id.clone(), dependency);

        let resolve = self
            .resolve
            .as_ref()
            .ok_or_else(|| Error::new("metadata has no resolve"))?;

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

        for dependency_package_id in &node.dependencies {
            result = self
                .resolve_with(result, dependency_package_id)
                .map_err(|err| Error::new("transitive dependency failure").with_source(err))?;
        }

        Ok(result)
    }
}
