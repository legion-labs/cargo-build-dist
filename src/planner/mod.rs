mod docker;
pub use docker::*;

mod copy;
use crate::metadata::Dependency;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub trait Action {
    fn run(&self, verbose: bool) -> Result<(), String>;
    fn dryrun(&self) -> Result<(), String>;
}

pub fn plan_build(context: &super::Context) -> Result<Vec<Box<dyn Action>>, String> {
    let mut actions: Vec<Box<dyn Action>> = vec![];
    //for docker_package in &context.docker_packages {
    //    actions.push(Box::new(Dockerfile::new(docker_package)?));
    //    actions.push(Box::new(CopyFiles::new(docker_package)?));
    //    actions.push(Box::new(DockerImage::new(docker_package)?));
    //}
    Ok(actions)
}

pub fn check_build_dependencies(context: &super::Context) -> Result<(), String> {
    println!("| Check package dependencies |");
    //for package in &context.docker_packages {
    //    let calculated_dependencies_hash = get_calculate_dependencies_hash(&package.dependencies);
    //    if let Some(deps_hash) = &package.docker_settings.deps_hash {
    //        if *deps_hash != calculated_dependencies_hash {
    //            return Err(format!("Package is NOT ready to be dockerized and pushed to the docker registry
    //            name: {},
    //            version: {}
    //            identified by the deps_hash: {}
    //            calculated deps_hash: {}.\nPlease update the version and deps_hash with the calculated deps_hash {} in the Cargo.toml of the package",
    //            package.name,
    //            package.version,
    //            deps_hash,
    //            &calculated_dependencies_hash,
    //            &calculated_dependencies_hash));
    //        } else {
    //            println!("Package is ready to be dockerized and pushed to the docker registry\n name:{},\n version:{}\n identified by the deps_hash:{}\n ",
    //            package.name,
    //            package.version,
    //            deps_hash);
    //        }
    //    } else {
    //        return Err("Error, the meta data deps_hash is not provided".to_string());
    //    }
    //}
    Ok(())
}
