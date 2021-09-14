mod docker;
pub use docker::*;

mod copy;
use crate::Dependency;
use copy::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub trait Action {
    fn run(&self, verbose: bool) -> Result<(), String>;
    fn dryrun(&self) -> Result<(), String>;
}

pub fn plan_build(context: &super::Context) -> Result<Vec<Box<dyn Action>>, String> {
    let mut actions: Vec<Box<dyn Action>> = vec![];
    for docker_package in &context.docker_packages {
        actions.push(Box::new(Dockerfile::new(docker_package)?));
        actions.push(Box::new(CopyFiles::new(docker_package)?));
        actions.push(Box::new(DockerImage::new(docker_package)?));
    }
    Ok(actions)
}

pub fn check_build_dependencies(context: &super::Context) -> Result<(), String> {
    println!("| Check package dependencies |");
    for package in &context.docker_packages {
        let calculated_dependencies_hash = get_calculate_dependencies_hash(&package.dependencies);
        if let Some(deps_hash) = &package.docker_settings.deps_hash {
            if *deps_hash != calculated_dependencies_hash {
                return Err(format!("Package is NOT ready to be dockerized and pushed to the docker registry
                name: {},
                version: {}
                identified by the deps_hash: {}
                calculated deps_hash: {}.\nPlease update the version and deps_hash with the calculated deps_hash {} in the Cargo.toml of the package", 
                package.name,
                package.version,
                deps_hash,
                &calculated_dependencies_hash,
                &calculated_dependencies_hash));
            } else {
                println!("Package is ready to be dockerized and pushed to the docker registry\n name:{},\n version:{}\n identified by the deps_hash:{}\n ",
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

fn get_calculate_dependencies_hash(dependencies: &BTreeSet<Dependency>) -> String {
    let mut deps_hasher = Sha256::new();
    for dep in dependencies {
        deps_hasher.update(&dep.name);
        deps_hasher.update(&dep.version);
    }
    format!("{:x}", deps_hasher.finalize())
}

pub fn push_builded_image(
    context: &super::Context,
    registry_type: &str,
    auto_create_repository: bool,
) -> Result<(), String> {
    match registry_type {
        "aws" => {
            push_image_to_aws(context, auto_create_repository)?;
        }
        _ => {
            return Err("Failed to push image, REGISTRY TYPE doesn't exists".to_string());
        }
    }
    Ok(())
}

/// Push the builded image and push it to AWS ECR
/// The function expects that the image is already builded locally
pub fn push_image_to_aws(
    context: &crate::Context,
    auto_create_repository: bool,
) -> Result<(), String> {
    println!("Push images to ECR");
    for package in &context.docker_packages {
        let name = &package.name;
        let version = &package.version;

        // verify that image identified by the tag NAME:VERSION exists locally first.
        let image = format!("{}:{}", &name, &version);
        if !image_exists_locally(&image) {
            return Err(format!(
                "Failed, image identified by the tag {} doesn't exists locally",
                &image
            ));
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let token = rt.block_on(docker::ecr::get_credentials_from_aws_ecr_authorization_token());
        let credentials = token.unwrap();

        let repository_exists = rt.block_on(docker::ecr::repository_exists(name.to_string()));
        if !repository_exists && auto_create_repository {
            rt.block_on(docker::ecr::create_repository(name.to_string()))?;
        }

        let target = format!("{}/{}:{}", &credentials.endpoint, &name, &version);

        //log into ECR
        exec_docker_command(
            [
                DOCKER_COMMAND_LOGIN,
                "--username",
                &credentials.username,
                "--password",
                &credentials.password,
                &credentials.endpoint,
            ]
            .to_vec(),
        )?;

        exec_docker_command([DOCKER_COMMAND_TAG, &image, &target].to_vec())?;

        exec_docker_command([DOCKER_COMMAND_PUSH, &target].to_vec())?;
    }
    Ok(())
}