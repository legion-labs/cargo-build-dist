use std::{collections::BTreeSet, path::PathBuf};

use serde::Deserialize;

use crate::{
    docker::{package::Dependency, DockerPackage, TargetDir},
    Error, Result,
};

#[derive(Debug, Clone, Deserialize)]
pub struct DockerMetadata {
    pub deps_hash: Option<String>,
    pub base: String,
    pub copy_dest_dir: String,
    pub env: Option<Vec<EnvironmentVariable>>,
    pub run: Option<Vec<String>>,
    pub expose: Option<Vec<i32>>,
    pub workdir: Option<String>,
    pub extra_copies: Option<Vec<CopyCommand>>,
    pub extra_commands: Option<Vec<String>>,
}

impl DockerMetadata {
    pub fn into_dist_target(
        self,
        target_dir: &PathBuf,
        package: &cargo_metadata::Package,
    ) -> Result<DockerPackage> {
        let docker_dir = target_dir.join("docker").join(&package.name);

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
            return Err(Error::new("package contain no binaries").with_explanation(format!("Building a Docker image requires at least one binary but the package {} does not contain any.", package.id)));
        }

        //let dependencies = get_transitive_dependencies(&metadata, package_id)?;
        let dependencies = BTreeSet::<Dependency>::new();

        Ok(DockerPackage {
            name: package.name.clone(),
            version: package.version.to_string(),
            toml_path: package.manifest_path.to_string(),
            binaries,
            metadata: self,
            dependencies,
            target_dir: TargetDir {
                binary_dir: target_dir.clone(),
                docker_dir,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopyCommand {
    pub source: String,
    pub destination: String,
}

//fn get_transitive_dependencies(
//    metadata: &cargo_metadata::Metadata,
//    package_id: &PackageId,
//) -> Result<BTreeSet<Dependency>> {
//    if metadata.resolve.is_none() {
//        bail!(
//            "resolve section not found in the workspace: {}",
//            metadata.workspace_root
//        );
//    }
//    // Can be unwrapped SAFELY after validating the not None resolve and being positively sure there is no error.
//    let resolve = metadata.resolve.as_ref().unwrap();
//
//    // accumulating all the resolved dependencies
//    let node = resolve.nodes.iter().find(|node| node.id == *package_id);
//    if node.is_none() {
//        bail!(
//            "failed to find the resolved dependencies for: {}",
//            package_id
//        );
//    }
//    // node can be unwrap SAFELY since we have validate it is not
//    let node = node.unwrap();
//    let mut deps = BTreeSet::new();
//    for dep_id in &node.dependencies {
//        let dep = &metadata[dep_id];
//
//        deps.insert(Dependency {
//            name: dep.name.clone(),
//            version: dep.version.to_string(),
//        });
//        deps.append(&mut get_transitive_dependencies(metadata, dep_id)?);
//    }
//
//    Ok(deps)
//}
//
