//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands, most notably
//! the tree of workspace members containing a package.metadata.docker entry

use std::path::{Path, PathBuf};
use std::{
    convert::{TryFrom, TryInto},
    vec,
};

use cargo_metadata::PackageId;
use serde::Deserialize;

use std::collections::HashSet;
use std::iter::FromIterator;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DockerMetadata {
    pub deps_hash: Option<String>,
    pub base: String,
    pub copy_dest_dir: String,
    pub env: Option<Vec<EnvironmentVariable>>,
    pub run: Option<Vec<String>>,
    pub expose: Option<Vec<i32>>,
    pub workdir: Option<String>,
    pub entrypoint: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Metadata {
    docker: Option<DockerMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopyCommand {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetDir {
    pub binary_dir: PathBuf,
    pub docker_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DockerSettings {
    pub deps_hash: Option<String>,
    pub base: String,
    pub copy_dest_dir: String,
    pub env: Option<Vec<EnvironmentVariable>>,
    pub run: Option<Vec<String>>,
    pub expose: Option<Vec<i32>>,
    pub workdir: Option<String>,
    pub entrypoint: Option<String>,
    pub user: Option<String>,
}

impl TryFrom<Metadata> for Option<DockerSettings> {
    type Error = String;

    fn try_from(value: Metadata) -> Result<Self, Self::Error> {
        if let Some(docker_metadata) = value.docker {
            // validate having base FROM.
            let base = &docker_metadata.base;
            if base.trim().is_empty() {
                return Err(format!("Container BASE cannot be empty"));
            }

            if let Some(user) = &docker_metadata.user {
                if user.trim().is_empty() {
                    return Err(format!("User cannot be empty"));
                }
            }

            if let Some(workdir) = &docker_metadata.workdir {
                if workdir.trim().is_empty() {
                    return Err(format!("Working directory cannot be empty"));
                }
            }

            if let Some(entrypoint) = &docker_metadata.entrypoint {
                if entrypoint.trim().is_empty() {
                    return Err(format!("Entrypoint cannot be empty"));
                }
            }

            if let Some(runs) = &docker_metadata.run {
                if runs.is_empty() {
                    return Err(format!("Runs commands cannot be empty"));
                } else {
                    for run in runs {
                        if run.trim().is_empty() {
                            return Err(format!("Run command cannot be empty"));
                        }
                    }
                }
            }

            if let Some(ports) = &docker_metadata.expose {
                if ports.is_empty() {
                    return Err(format!("Port cannot be empty"));
                }
            }

            // validate COPY commands
            let copy_dest_dir = &docker_metadata.copy_dest_dir;
            if copy_dest_dir.trim().is_empty() {
                return Err(format!("Copy destination directory cannot be empty"));
            }

            Ok(Some(DockerSettings {
                deps_hash: docker_metadata.deps_hash,
                base: docker_metadata.base,
                workdir: docker_metadata.workdir,
                entrypoint: docker_metadata.entrypoint,
                expose: docker_metadata.expose,
                run: docker_metadata.run,
                env: docker_metadata.env,
                copy_dest_dir: docker_metadata.copy_dest_dir,
                user: docker_metadata.user,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub docker_settings: DockerSettings,

    pub dependencies: HashSet<Dependency>,
    pub target_dir: TargetDir,
}

pub struct Context {
    pub target_dir: PathBuf,
    pub docker_packages: Vec<DockerPackage>,
}

impl Context {
    /// Building a context regardless of the planning and execution
    pub fn build(cargo: &str, is_debug: bool) -> Result<Self, String> {
        let mut cmd = cargo_metadata::MetadataCommand::new();
        // even if MetadataCommand::new() can find cargo using the env var
        // we don't want to run that logic twice
        cmd.cargo_path(cargo);
        // todo support --manifest-path

        let metadata = cmd.exec();
        if let Err(e) = &metadata {
            return Err(format!("failed to run cargo metadata {}", e));
        }
        let metadata = metadata.unwrap();

        let mut target_dir = PathBuf::new();
        target_dir.push(metadata.target_directory.as_path());
        target_dir.push(if is_debug { "debug" } else { "release" });

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
            let docker_settings: Result<Option<DockerSettings>, String> =
                docker_metadata.unwrap().try_into();
            if let Err(e) = &docker_settings {
                return Err(format!("failed to parse the docker metadata: {}", e));
            } else if let Ok(None) = &docker_settings {
                continue;
            };
            // We can safely unwrap here, we know the data is sane
            let docker_settings = docker_settings.unwrap().unwrap();

            // We need all the binaries so we package them later on
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

            let dependencies = get_transitive_dependencies(&metadata, package_id)?;
            let dependencies = dependency_hash_to_set(dependencies);

            let mut docker_dir = PathBuf::new();
            docker_dir.push(target_dir.clone());
            docker_dir.push("docker");
            docker_dir.push(package.name.clone());

            docker_packages.push(DockerPackage {
                name: package.name.clone(),
                version: package.version.to_string(),
                toml_path: package.manifest_path.to_string(),
                binaries,
                docker_settings,
                dependencies,
                target_dir: TargetDir {
                    binary_dir: target_dir.clone(),
                    docker_dir: docker_dir,
                },
            })
        }

        //let docker_packages_str = format!("{:?}", docker_packages);
        //println!("{}", docker_packages_str);

        Ok(Context {
            target_dir,
            docker_packages,
        })
    }
}

fn get_transitive_dependencies(
    metadata: &cargo_metadata::Metadata,
    package_id: &PackageId,
) -> Result<Vec<Dependency>, String> {
    if metadata.resolve.is_none() {
        return Err(format!(
            "resolve section not found in the workspace: {}",
            metadata.workspace_root
        ));
    }
    let resolve = metadata.resolve.as_ref().unwrap();

    // accumulating all the resolved dependencies
    let node = resolve.nodes.iter().find(|node| node.id == *package_id);
    if node.is_none() {
        return Err(format!(
            "failed to find the resolved dependencies for: {}",
            package_id
        ));
    }
    let node = node.unwrap();

    let mut deps = vec![];
    for dep_id in &node.dependencies {
        let dep = &metadata[dep_id];
        deps.push(Dependency {
            name: dep.name.clone(),
            version: dep.version.to_string(),
        });
        deps.append(&mut get_transitive_dependencies(metadata, dep_id)?);
    }

    Ok(deps)
}

fn dependency_hash_to_set(vec: Vec<Dependency>) -> HashSet<Dependency> {
    HashSet::from_iter(vec)
}
