pub(crate) mod ecr;
//use ecr::*;

use crate::{Action, DockerPackage};
use itertools::Itertools;
use std::{path::PathBuf, process::Command, str};

const DOCKER_FILE_NAME: &str = "Dockerfile";

pub const DOCKER_COMMAND: &str = "docker";
pub const DOCKER_COMMAND_PUSH: &str = "push";
pub const DOCKER_COMMAND_BUILD: &str = "build";
pub const DOCKER_COMMAND_TAG: &str = "tag";
pub const DOCKER_COMMAND_LOGIN: &str = "login";
pub const DOCKER_COMMAND_IMAGE: &str = "image";

pub struct Dockerfile {
    content: String,
    path: PathBuf,
}

impl Dockerfile {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let setting = &docker_package.docker_settings;
        let mut content = format!("FROM {}\n", &setting.base);
        if let Some(variables) = &setting.env {
            let env_variables: Vec<String> = variables
                .iter()
                .filter(|var| !var.name.is_empty() && !var.value.is_empty())
                .map(|var| format!("{}={}", var.name, var.value))
                .collect();
            content.push_str(&format!("ENV {}\n", &env_variables.iter().join(" \\\n")));
        }
        for binary in &docker_package.binaries {
            content.push_str(&format!("COPY {} {}\n", binary, &setting.copy_dest_dir));
        }

        if let Some(extra_copies) = &setting.extra_copies {
            for copy in extra_copies {
                let file_path = copy.source.split('/');
                let names: Vec<&str> = file_path.collect();
                let file_name = names.last().expect("File extension cannot be read");
                content.push_str(&format!("COPY {} {}\n", file_name, copy.destination));
            }
        }
        if let Some(extra_commands) = &setting.extra_commands {
            for command in extra_commands {
                content.push_str(&format!("{}\n", command));
            }
        }
        if let Some(ports) = &setting.expose {
            content.push_str(&format!("EXPOSE {}\n", &ports.iter().join(" ")));
        }
        if let Some(workdir) = &setting.workdir {
            content.push_str(&format!("WORKDIR {}\n", workdir));
        }

        content.push_str(&format!("CMD [\"./{}\"]", &docker_package.binaries[0]));

        Ok(Self {
            content,
            path: docker_package.target_dir.docker_dir.join(DOCKER_FILE_NAME),
        })
    }
}

impl Action for Dockerfile {
    fn run(&self, verbose: bool) -> Result<(), String> {
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                if verbose {
                    println!(
                        "Folder {} doesn't exists, let create it",
                        &docker_dir.display()
                    );
                }
                if let Err(e) = std::fs::create_dir_all(&docker_dir) {
                    return Err(format!(
                        "Error creating directory {}: {}",
                        docker_dir.display(),
                        e
                    ));
                }
            }
        }

        if verbose {
            println!("Create the file {}", &self.path.display());
        }
        if let Err(e) = std::fs::write(&self.path, &self.content) {
            return Err(format!(
                "Failed to write docker file {}:{}",
                &self.path.display(),
                e
            ));
        }
        Ok(())
    }

    fn dryrun(&self) -> Result<(), String> {
        println!("| Create Dockerfile |");
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                println!("Create directory {}", docker_dir.display());
            }
        }
        println!(
            "File location:\n{} \nFile Content:\n{} ",
            self.path.display(),
            self.content
        );

        Ok(())
    }
}

pub struct DockerImage {
    name: String,
    tag: String,
    dockerfile_path: PathBuf,
}

impl DockerImage {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let dockerfile_path = PathBuf::from(&docker_package.target_dir.docker_dir);
        Ok(Self {
            name: docker_package.name.clone(),
            tag: docker_package.version.clone(),
            dockerfile_path,
        })
    }

    pub fn get_docker_build_args(&self) -> Vec<String> {
        [
            DOCKER_COMMAND_BUILD.to_string(),
            "-t".to_string(),
            format!("{}:{}", &self.name, &self.tag),
            ".".to_string(),
        ]
        .to_vec()
    }
}

impl Action for DockerImage {
    fn run(&self, verbose: bool) -> Result<(), String> {
        if verbose {
            println!("Execute docker {}", &self.get_docker_build_args().join(" "));
        }
        //exec_docker_command(self.get_docker_build_args().iter().map(String::as_str).collect())?;
        let status = Command::new(DOCKER_COMMAND)
            .args(&self.get_docker_build_args())
            .current_dir(&self.dockerfile_path)
            .status()
            .expect("Failed to execute docker command");
        if !status.success() {
            return Err(format!(
                "Failed to execute command docker with args {}",
                &self.get_docker_build_args().join(" ")
            ));
        }
        Ok(())
    }

    // implement the dry run
    fn dryrun(&self) -> Result<(), String> {
        println!("| Build DockerImage |");
        println!(
            "From:\n{}\nExecute command:\n{} ",
            &self.dockerfile_path.display(),
            self.get_docker_build_args().join(" ")
        );
        Ok(())
    }
}

pub fn exec_docker_command(args: Vec<&str>) -> Result<(), String> {
    let status = Command::new(DOCKER_COMMAND)
        .args(args)
        .status()
        .expect("Failed to execute docker command");
    if !status.success() {
        return Err("Failed to execute command docker with args".to_string());
    }
    Ok(())
}

pub fn image_exists_locally(id: &str) -> bool {
    let output = Command::new("docker")
        .arg(DOCKER_COMMAND_IMAGE)
        .arg("ls")
        .arg("--format")
        .arg("{{json .ID}}")
        .arg(&id)
        .output()
        .expect("Failed to execute docker image ls");
    let s = str::from_utf8(&output.stdout).unwrap();
    !s.is_empty()
}
