mod docker;
use docker::*;

mod copy;
use copy::*;
use sha2::{Digest, Sha256};

pub trait Action {
    fn run(&self) -> Result<(), String>;
    fn dryrun(&self) -> Result<(), String>;
}

pub fn plan_build(context: &super::Context) -> Result<Vec<Box<dyn Action>>, String> {
    let mut actions: Vec<Box<dyn Action>> = vec![];
    for docker_package in &context.docker_packages {
        actions.push(Box::new(Dockerfile::new(docker_package)?));
        actions.push(Box::new(CopyFiles::new(docker_package)?));
        actions.push(Box::new(DockerImage::new(docker_package)?))
    }
    Ok(actions)
}

pub fn check_build_dependencies(context: &super::Context) -> Result<(), String> {
    for package in &context.docker_packages {
        let mut deps_hasher = Sha256::new();
        for dep in &package.dependencies {
            deps_hasher.update(&dep.name);
            deps_hasher.update(&dep.version);
        }
        if let Some(deps_hash) = &package.docker_settings.deps_hash {
            let calculate_deps_hash = format!("{:x}", deps_hasher.finalize());
            if calculate_deps_hash != deps_hash.to_string() {
                return Err(format!("Failed, deps_hash:{} defined in the Cargo.toml file, is not equivalent to the calculated dependencies: {}",
                deps_hash.to_string(),
                calculate_deps_hash));
            } else {
                println!("Package is ready to be dockerized and deployed to the docker registry\n name:{},\n version:{}\n identified by the deps_hash:{}\n ", 
                package.name, 
                package.version, 
                deps_hash);
            }
        } else {
            return Err("Error, the meta data deps_hash is not provided".to_string());
        }
    }
    Ok(())
}

pub fn deploy_build(_context: &super::Context) -> Result<(), String> {
    Ok(())
}
