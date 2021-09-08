mod docker;
use docker::*;

mod copy;
use crate::{Dependency};
use copy::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::process::Command;
use std::str;

pub trait Action {
    fn run(&self, verbose: bool) -> Result<(), String>;
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
    println!("------------------------------");
    println!("| Check package dependencies |");
    println!("------------------------------");
    for package in &context.docker_packages {
        let calculated_dependencies_hash = get_calculate_dependencies_hash(&package.dependencies);
        if let Some(deps_hash) = &package.docker_settings.deps_hash {
            if deps_hash.to_string() != calculated_dependencies_hash {
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
    registry_type: String,
    auto_create_repository: bool,
) -> Result<(), String> {
    match registry_type.as_str() {
        "aws" => {
            push_builded_image_to_aws(&context, auto_create_repository)?;
        }
        _ => {
            return Err(format!(
                "Failed to push image, REGISTRY TYPE doesn't exists"
            ));
        }
    }
    Ok(())
}

fn push_builded_image_to_aws(
    context: &super::Context,
    auto_create_repository: bool,
) -> Result<(), String> {
    println!("Push image to AWS ECR");
    for package in &context.docker_packages {
        let name = &package.name;
        let version = &package.version;

        let image = format!("{}:{}", &name, &version);
        if !image_exists_locally(&image) {
            return Err(format!("Failed, image {} doesn't exists locally", &image));
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let token_credentials = rt.block_on(get_credentials_from_aws_ecr_authorization_token());

        let credentials = token_credentials.unwrap();
        let repo_already_exists = rt.block_on(ecr_repository_already_exists(name.to_string()));
        if !repo_already_exists && auto_create_repository {
            rt.block_on(ecr_create_repository(name.to_string()))?;
        }
        ecr_login(
            credentials.username,
            credentials.password,
            credentials.endpoint.clone(),
        )?;
        image_tag_for_ecr(&image, &name, &version, &credentials.endpoint)?;
        image_push_to_ecr(&name, &version, &credentials.endpoint)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct TokenCredentials {
    username: String,
    password: String,
    endpoint: String,
}
impl TokenCredentials {
    fn new(token: String, endpoint: String) -> Result<Self, String> {
        let bytes = base64::decode(token).unwrap();
        let decoded_token = std::str::from_utf8(&bytes).unwrap();
        let basic_credentials = decoded_token.split(":");
        let credentials: Vec<&str> = basic_credentials.collect();
        if credentials.is_empty() {
            return Err("Cannot find credentials".to_string());
        }
        Ok(Self {
            username: credentials[0].to_string(),
            password: credentials[1].to_string(),
            endpoint: endpoint,
        })
    }
}

async fn get_credentials_from_aws_ecr_authorization_token() -> Result<TokenCredentials, String> {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client.get_authorization_token().send().await;
    match resp {
        Ok(s) => {
            if let Some(data) = s.authorization_data {
                let authorization = data.first().unwrap();
                let ecr_endpoint = authorization
                    .proxy_endpoint
                    .as_ref()
                    .unwrap()
                    .replace("https://", "");
                let token = authorization.authorization_token.as_ref().unwrap();
                Ok(TokenCredentials::new(token.clone(), ecr_endpoint).unwrap())
            } else {
                Err(format!("Fail to deseriazlize Authorization data"))
            }
        }
        Err(e) => Err(format!("Failed to get ecr authorization token {}", e)),
    }
}

async fn ecr_repository_already_exists(name: String) -> bool {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client.describe_repositories().send().await;
    let describe_repositories = resp.unwrap();
    for repository in describe_repositories.repositories.unwrap() {
        if repository.repository_name.unwrap() == name {
            return true;
        }
    }
    return false;
}

async fn ecr_create_repository(name: String) -> Result<(), String> {
    let client = aws_sdk_ecr::Client::from_env();
    let resp = client
        .create_repository()
        .repository_name(&name)
        .send()
        .await;
    match resp {
        Ok(result) => {
            let repository = result.repository.unwrap();
            println!(
                "Repository {} is create",
                repository.repository_name.unwrap()
            );
            Ok(())
        }
        Err(e) => Err(format!("Failed to create repository {} : {}", &name, e)),
    }
}

fn image_exists_locally(id: &String) -> bool {
    let output = Command::new("docker")
        .arg("image")
        .arg("ls")
        .arg("--format")
        .arg("{{json .ID}}")
        .arg(&id)
        .output()
        .expect("Failed to execute docker image ls");
    let s = str::from_utf8(&output.stdout).unwrap();
    !s.is_empty()
}

fn image_push_to_ecr(name: &String, tag: &String, ecr_endpoint: &String) -> Result<(), String> {
    let target = format!("{}/{}:{}", ecr_endpoint, name, tag);
    // docker push AWS_ACCOUNT_ID.dkr.ecr.REGION.amazonaws.com/IMAGENAME:TAG
    let status = Command::new("docker")
        .arg("push")
        .arg(&target)
        .status()
        .expect("Failed to execute docker push command");
    if !status.success() {
        return Err(format!("Failed to push docker image on {}", target));
    }
    Ok(())
}

fn image_tag_for_ecr(
    id: &String,
    name: &String,
    tag: &String,
    ecr_endpoint: &String,
) -> Result<(), String> {
    // docker tag IMAGE_ID AWS_ACCOUNT_ID.dkr.ecr.REGION.amazonaws.com/IMAGENAME:TAG
    let target = format!("{}/{}:{}", ecr_endpoint, name, tag);
    let status = Command::new("docker")
        .arg("tag")
        .arg(id)
        .arg(&target)
        .status()
        .expect("Failed to execute docker tag command");
    if !status.success() {
        return Err(format!("Failed to tag docker image {}  {}", id, target));
    }
    Ok(())
}

fn ecr_login(username: String, password: String, endpoint: String) -> Result<(), String> {
    let status = Command::new("docker")
        .arg("login")
        .arg("--username")
        .arg(&username)
        .arg("--password")
        .arg(password)
        .arg(&endpoint)
        .status()
        .expect("Failed to execute docker login command");
    if !status.success() {
        return Err(format!(
            "Failed to login to AWS ECR on {} with user {}",
            &endpoint, username
        ));
    }
    Ok(())
}

pub fn deploy_build_not_implement() -> Result<(), String> {
    println!("Please implement the registry type");
    Ok(())
}
