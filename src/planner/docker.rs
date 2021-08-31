use crate::{Action, CopyCommand, DockerPackage, EnvironmentVariable};
use itertools::Itertools;
use std::{path::PathBuf, process::Command};

const DOCKER_TEMPLATE_NAME: &str = "Dockerfile";

pub struct Dockerfile {
    content: String,
    path: PathBuf,
}

impl Dockerfile {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        if let Ok(template) = tera::Template::new(
            DOCKER_TEMPLATE_NAME,
            None,
            include_str!("../templates/Dockerfile.template"),
        ) {
            let mut tera = tera::Tera::default();
            tera.set_escape_fn(escape_docker);
            tera.autoescape_on(vec![DOCKER_TEMPLATE_NAME]);
            tera.templates
                .insert(DOCKER_TEMPLATE_NAME.to_string(), template);

            let mut context = tera::Context::new();

            // based on the dockersettings, we need to integrate the necessary docker commands
            // into the dockerfile.

            // FROM command
            let docker_setting = &docker_package.docker_settings;
            context.insert("base", &docker_setting.base);

            // ENV command
            if let Ok(_str) = build_env_variables_command_str(&docker_setting.env) {
                context.insert("env_variable", &_str);
            }

            // COPY command(s) for extra copy
            let mut copy_cmd = String::from(build_copy_command_str(&docker_package.binaries, &docker_setting.copy_dest_dir));
            copy_cmd.push_str(&build_extra_copies_command_str(&docker_setting.extra_copies));
            context.insert("copy_cmd", &copy_cmd);

            // // RUN command(s)
            // if let Ok(_str) = build_run_command_str(&docker_setting.run) {
            //     context.insert("run_cmd", &_str);
            // }

            // // ADD USER command
            // let mut user_cmd_str = String::new();
            // if let Some(user) = &docker_setting.user {
            //     user_cmd_str.push_str("USER ");
            //     user_cmd_str.push_str(user);
            // }
            // context.insert("user_cmd", &user_cmd_str);

            // WORKDIR command
            let mut wordir_cmd_str = String::new();
            if let Some(workdir) = &docker_setting.workdir {
                wordir_cmd_str.push_str("WORKDIR ");
                wordir_cmd_str.push_str(workdir);
            }
            context.insert("workdir_cmd", &wordir_cmd_str);

            // EXPOSE command
            let mut expose_command_str = String::new();
            if let Some(ports) = &docker_setting.expose {
                if !ports.is_empty() {
                    expose_command_str.push_str("EXPOSE ");
                    let ports_str = ports.iter().join(" ");
                    expose_command_str.push_str(&ports_str);
                }
            }
            context.insert("expose_cmd", &expose_command_str);

            context.insert("executable", &docker_package.binaries[0]);

            if let Ok(content) = tera.render(DOCKER_TEMPLATE_NAME, &context) {
                let mut docker_file_path =
                    PathBuf::from(docker_package.target_dir.docker_dir.clone());
                docker_file_path.push(DOCKER_TEMPLATE_NAME.to_string());
                Ok(Self {
                    content,
                    path: docker_file_path,
                })
            } else {
                Err("Failed to render template file".to_string())
            }
        } else {
            Err("Failed to parse template file".to_string())
        }
    }
}

impl Action for Dockerfile {
    fn run(&self) -> Result<(), String> {
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(docker_dir) {
                    return Err(format!(
                        "Error creating directory {}: {}",
                        docker_dir.display(),
                        e
                    ));
                }
            }
        }

        match std::fs::write(&self.path, &self.content) {
            Err(e) => {
                return Err(format!(
                    "Failed to write docker file {}:{}",
                    &self.path.display(),
                    e
                ))
            }
            _ => (),
        }
        Ok(())
    }

    fn dryrun(&self) -> Result<(), String> {
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                println!("Create directory {}", docker_dir.display());
            }
        }
        println!("Creating the file {}", self.path.display());
        println!("With content: \n{}", self.content);
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
            dockerfile_path: dockerfile_path,
        })
    }
}

impl Action for DockerImage {
    fn run(&self) -> Result<(), String> {
        let docker_build = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(format!("{}:{}", &self.name, &self.tag))
            .arg(".")
            .current_dir(&self.dockerfile_path)
            .status()
            .expect("Failed to execute docker command");
        if !docker_build.success() {
            return Err(format!("Problem to build docker image"));
        }
        Ok(())
    }

    // implement the dry run
    fn dryrun(&self) -> Result<(), String> {
        println!("Execute command:");
        println!();
        Ok(())
    }
}

fn build_env_variables_command_str(
    env_variables: &Option<Vec<EnvironmentVariable>>,
) -> Result<String, String> {
    let mut env_variables_command_str = String::new();
    if let Some(variables) = env_variables {
        env_variables_command_str.push_str("ENV ");
        let env_variables: Vec<String> = variables
            .iter()
            .filter(|var| !var.name.is_empty() && !var.value.is_empty())
            .map(|var| format!("{}={}", var.name, var.value))
            .collect();
        env_variables_command_str.push_str(&env_variables.iter().join(" \\\n"));
    }
    Ok(env_variables_command_str)
}

fn build_run_command_str(run_cmd: &Option<Vec<String>>) -> String {
    let mut run_command_str = String::new();
    if let Some(runs) = run_cmd {
        run_command_str.push_str("RUN ");
        run_command_str.push_str(&runs.iter().join(" \\\n"));
    }
    run_command_str
}

fn build_copy_command_str(sources: &Vec<String>, destination_dir: &String) -> String {
    let mut copy_command_str = String::new();
    for source in sources {
        copy_command_str.push_str("COPY ");
        copy_command_str.push_str(&format!("{} {} \n\n", source, destination_dir))
    }
    copy_command_str
}

fn build_extra_copies_command_str(copies_command: &Option<Vec<CopyCommand>>) -> String {
    let mut copy_command_str = String::new();
    if let Some(copies_command) = copies_command{
        for command in copies_command {
            let filepath = command.source.split("/");
            let names: Vec<&str> = filepath.collect();
            let filename = names.last().expect("File extension cannot be read");
            copy_command_str.push_str("COPY ");
            copy_command_str.push_str(&format!("{} {} \n\n", filename, command.destination))
        }
    }
    copy_command_str
}

fn escape_docker(input: &str) -> String {
    let mut output = String::with_capacity(input.len() * 2);
    for c in input.chars() {
        match c {
            //'\n' => output.push_str("\\"),
            '\r' => output.push_str(""),
            _ => output.push(c),
        }
    }

    // Not using shrink_to_fit() on purpose
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_copy_command_str() {
        let sources: Vec<String> = vec!["f1.txt".to_string(), "other/f2.txt".to_string()];
        let container_destination_dir = "/usr/src/app";

        let copy_str = build_copy_command_str(&sources, &container_destination_dir.to_string());
        let t1_str = "COPY f1.txt usr/src/app/f1.txt";
        let t2_str = "COPY f2.txt /usr/src/app/other/f2.txt";
        assert_eq!(true, copy_str.contains(t1_str) && copy_str.contains(t2_str));
    }
}
