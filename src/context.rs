//! Gathers all the environment information and build a Context containing
//! all relevant information for the rest of the commands, most notably
//! the tree of workspace members containing a package.metadata.docker entry

use cargo_metadata::PackageId;
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::BTreeSet,
    convert::{TryFrom, TryInto},
    path::PathBuf
};

#[derive(Debug, Eq, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
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

#[derive(Debug, Clone, Deserialize)]
struct DockerMetadata {
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
    pub extra_copies: Option<Vec<CopyCommand>>,
    pub extra_commands: Option<Vec<String>>,
}

impl TryFrom<Metadata> for Option<DockerSettings> {
    type Error = String;

    fn try_from(value: Metadata) -> Result<Self, Self::Error> {
        if let Some(docker_metadata) = value.docker {
            // validate having base FROM.
            let base = &docker_metadata.base;
            if base.trim().is_empty() {
                return Err("Container BASE cannot be empty".to_string());
            }

            if let Some(workdir) = &docker_metadata.workdir {
                if workdir.trim().is_empty() {
                    return Err("Working directory cannot be empty".to_string());
                }
            }

            if let Some(runs) = &docker_metadata.run {
                if runs.is_empty() {
                    return Err("Runs commands cannot be empty".to_string());
                } else {
                    for run in runs {
                        if run.trim().is_empty() {
                            return Err("Run command cannot be empty".to_string());
                        }
                    }
                }
            }

            if let Some(ports) = &docker_metadata.expose {
                if ports.is_empty() {
                    return Err("Port cannot be empty".to_string());
                }
            }

            let copy_dest_dir = &docker_metadata.copy_dest_dir;
            if copy_dest_dir.trim().is_empty() {
                return Err("Copy destination directory cannot be empty".to_string());
            }

            if let Some(extra_copies) = &docker_metadata.extra_copies {
                if extra_copies.is_empty() {
                    return Err("Extra copies should not be empty if declared".to_string());
                } else {
                    for extra_copy in extra_copies {
                        if extra_copy.source.is_empty() {
                            return Err("Extra copy source cannot be empty".to_string());
                        }
                    }
                }
            }

            Ok(Some(DockerSettings {
                deps_hash: docker_metadata.deps_hash,
                base: docker_metadata.base,
                workdir: docker_metadata.workdir,
                expose: docker_metadata.expose,
                run: docker_metadata.run,
                env: docker_metadata.env,
                copy_dest_dir: docker_metadata.copy_dest_dir,
                extra_copies: docker_metadata.extra_copies,
                extra_commands: docker_metadata.extra_commands,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub docker_settings: DockerSettings,
    pub dependencies: BTreeSet<Dependency>,
    pub target_dir: TargetDir,
}

pub struct Context {
    pub target_dir: PathBuf,
    pub docker_packages: Vec<DockerPackage>,
    pub manifest_path: Option<PathBuf>,
}

impl Context {
    /// Building a context regardless of the planning and execution
    pub fn build(
        cargo: &str,
        is_debug_mode: bool,
        manifest_path: Option<&str>,
    ) -> Result<Self, String> {
        let mut cmd = cargo_metadata::MetadataCommand::new();
        // even if MetadataCommand::new() can find cargo using the env var
        // we don't want to run that logic twice
        cmd.cargo_path(cargo);

        // todo support --manifest-path
        let mut path = PathBuf::new();
        if let Some(manifest_path) = manifest_path {
            path.push(manifest_path);
            if !path.exists() {
                return Err(format!(
                    "failed to use the manifest file, {} doesn't exists",
                    &path.display()
                ));
            }
            cmd.manifest_path(&path);
        }

        let metadata = cmd.exec();
        if let Err(e) = &metadata {
            return Err(format!("failed to run cargo metadata {}", e));
        }
        let metadata = metadata.unwrap();

        let target_dir =
            PathBuf::from(metadata.target_directory.as_path().join(if is_debug_mode {
                "debug"
            } else {
                "release"
            }));

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

            let docker_dir = PathBuf::from(&target_dir.join("docker").join(&package.name));

            let dependencies = get_transitive_dependencies(&metadata, package_id)?;

            docker_packages.push(DockerPackage {
                name: package.name.clone(),
                version: package.version.to_string(),
                toml_path: package.manifest_path.to_string(),
                binaries,
                docker_settings,
                dependencies,
                target_dir: TargetDir {
                    binary_dir: target_dir.clone(),
                    docker_dir,
                },
            });
        }
        Ok(Self {
            target_dir,
            docker_packages,
            manifest_path: Some(path),
        })
    }
}

fn get_transitive_dependencies(
    metadata: &cargo_metadata::Metadata,
    package_id: &PackageId,
) -> Result<BTreeSet<Dependency>, String> {
    if metadata.resolve.is_none() {
        return Err(format!(
            "resolve section not found in the workspace: {}",
            metadata.workspace_root
        ));
    }
    // Can be unwrapped SAFELY after validating the not None resolve and being positively sure there is no error.
    let resolve = metadata.resolve.as_ref().unwrap();

    // accumulating all the resolved dependencies
    let node = resolve.nodes.iter().find(|node| node.id == *package_id);
    if node.is_none() {
        return Err(format!(
            "failed to find the resolved dependencies for: {}",
            package_id
        ));
    }
    // node can be unwrap SAFELY since we have validate it is not
    let node = node.unwrap();
    let mut deps = BTreeSet::new();
    for dep_id in &node.dependencies {
        let dep = &metadata[dep_id];

        deps.insert(Dependency {
            name: dep.name.clone(),
            version: dep.version.to_string(),
        });
        deps.append(&mut get_transitive_dependencies(metadata, dep_id)?);
    }

    Ok(deps)
}
