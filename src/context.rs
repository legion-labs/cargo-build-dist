//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands, most notably
//! the tree of workspace members containing a package.metadata.docker entry

use std::convert::{TryFrom, TryInto};

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DockerMetadata {
    pub deps_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Metadata {
    docker: Option<DockerMetadata>,
}

#[derive(Debug, Clone)]
pub struct DockerSettings {
    pub deps_hash: String,
}

impl TryFrom<Metadata> for Option<DockerSettings> {
    type Error = String;

    fn try_from(value: Metadata) -> Result<Self, Self::Error> {
        if let Some(_) = value.docker {
            // todo: validate input and return errors.
            Ok(Some(DockerSettings {
                deps_hash: "aa".to_string(),
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DockerPacakge {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub docker_settings: DockerSettings,
    pub deps: Vec<Dependency>,
}

pub struct Context {
    pub target_dir: String,
    pub docker_packages: Vec<DockerPacakge>,
}

impl Context {
    pub fn build(cargo: &str) -> Result<Self, String> {
        let mut cmd = cargo_metadata::MetadataCommand::new();
        // even if MetadataCommand::new() can find cargo using the env var
        // we don't want to run that logic twice
        cmd.cargo_path(cargo);
        // todo support --manifest-path

        let metadata = cmd.exec();
        if let Err(e) = &metadata {
            return Err(format!("failed to run cargo manifest {}", e));
        }
        let metadata = metadata.unwrap();
        if metadata.resolve.is_none() {
            return Err(format!(
                "resolve section not found in the workspace: {}",
                metadata.workspace_root
            ));
        }
        let resolve = metadata.resolve.as_ref().unwrap();
        let mut docker_packages = vec![];
        // for each workspace member, we're going to build a DockerPackage
        // contains binaries
        for package_id in &metadata.workspace_members {
            let package = &metadata[package_id];

            // Early out when we don't have metadata
            if package.metadata.is_null() {
                continue;
            }
            let docker_metadata = Metadata::deserialize(&package.metadata);
            if let Err(e) = &docker_metadata {
                return Err(format!("failed to deserialize docker metadata {}", e));
            }
            let docker_metadata = docker_metadata.unwrap();

            let docker_settings: Result<Option<DockerSettings>, String> =
                docker_metadata.try_into();
            if let Err(e) = &docker_settings {
                return Err(format!("failed to parse the docker metadata: {}", e));
            } else if let Ok(None) = &docker_settings {
                continue;
            };
            let docker_settings = docker_settings.unwrap().unwrap();

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
                return Err(format!(
                    "Docker metadata was found in {}, but no binaries were found in the crate",
                    package_id
                ));
            }
            let node = resolve.nodes.iter().find(|node| node.id == *package_id);
            if node.is_none() {
                return Err(format!(
                    "failed to find the resolved dependencies for: {}",
                    package_id
                ));
            }
            let node = node.unwrap();
            let deps: Vec<_> = node
                .dependencies
                .iter()
                .map(|dep_id| {
                    let dep = &metadata[dep_id];
                    Dependency {
                        name: dep.name.clone(),
                        version: dep.version.to_string(),
                    }
                })
                .collect();

            docker_packages.push(DockerPacakge {
                name: package.name.clone(),
                version: package.version.to_string(),
                toml_path: package.manifest_path.to_string(),
                binaries,
                docker_settings,
                deps,
            })
        }
        println!("{:?}", docker_packages);
        Ok(Context {
            target_dir: metadata.target_directory.to_string(),
            docker_packages,
        })
    }
}
