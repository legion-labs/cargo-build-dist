use crate::{Action, CopyCommand, DockerPackage, EnvironmentVariable};
use itertools::Itertools;
use std::{path::PathBuf, process::Command};

const DOCKER_TEMPLATE_NAME: &str = "Dockerfile";
const DOCKER_TEMPLATE_KEY_BASE: &str = "base";
const DOCKER_TEMPLATE_KEY_ENV: &str = "environment";
const DOCKER_TEMPLATE_KEY_RUN: &str = "run";
const DOCKER_TEMPLATE_KEY_COPY: &str = "copy";
const DOCKER_TEMPLATE_KEY_WORKDIR: &str = "workdir";
const DOCKER_TEMPLATE_KEY_EXPOSE: &str = "expose";
const DOCKER_TEMPLATE_KEY_EXECUTABLE: &str = "executable";

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
            let docker_setting = &docker_package.docker_settings;

            context.insert(DOCKER_TEMPLATE_KEY_BASE, &docker_setting.base);
            context.insert(
                DOCKER_TEMPLATE_KEY_ENV,
                &build_env_variables_command_str(&docker_setting.env),
            );
            let mut copy_cmd = String::from(build_copy_command_str(
                &docker_package.binaries,
                &docker_setting.copy_dest_dir,
            ));
            copy_cmd.push_str(&build_extra_copies_command_str(
                &docker_setting.extra_copies,
            ));
            context.insert(DOCKER_TEMPLATE_KEY_COPY, &copy_cmd);

            context.insert(
                DOCKER_TEMPLATE_KEY_RUN,
                &build_run_command_str(&docker_setting.run),
            );
            context.insert(
                DOCKER_TEMPLATE_KEY_WORKDIR,
                &build_workdir_command_str(&docker_setting.workdir),
            );
            context.insert(
                DOCKER_TEMPLATE_KEY_EXPOSE,
                &build_expose_command_str(&docker_setting.expose),
            );
            context.insert(DOCKER_TEMPLATE_KEY_EXECUTABLE, &docker_package.binaries[0]);

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
        println!("--------------------");
        println!("| Create Dockerfile |");
        println!("--------------------");
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
            dockerfile_path: dockerfile_path,
        })
    }
}

impl Action for DockerImage {
    fn run(&self, verbose: bool) -> Result<(), String> {
        let docker_build = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(format!("{}:{}", &self.name, &self.tag))
            .arg(".")
            .current_dir(&self.dockerfile_path)
            .status()
            .expect("Failed to execute docker build command");
        if !docker_build.success() {
            return Err(format!("Problem to build docker image"));
        }
        Ok(())
    }

    // implement the dry run
    fn dryrun(&self) -> Result<(), String> {
        println!("---------------------");
        println!("| Build DockerImage |");
        println!("---------------------");
        let build_args = [
            "docker",
            "build",
            "-t",
            &format!("{}:{}", &self.name, &self.tag),
            ".",
        ];
        let build_from = &self.dockerfile_path;

        println!(
            "From:\n{}\nExecute command:\n{} ",
            build_from.display(),
            build_args.join(" ")
        );
        Ok(())
    }
}

fn build_env_variables_command_str(env_variables: &Option<Vec<EnvironmentVariable>>) -> String {
    let mut cmd_str = String::new();
    if let Some(variables) = env_variables {
        cmd_str.push_str("ENV ");
        let env_variables: Vec<String> = variables
            .iter()
            .filter(|var| !var.name.is_empty() && !var.value.is_empty())
            .map(|var| format!("{}={}", var.name, var.value))
            .collect();
        cmd_str.push_str(&env_variables.iter().join(" \\\n"));
    }
    cmd_str
}

fn build_run_command_str(run_cmd: &Option<Vec<String>>) -> String {
    let mut cmd_str = String::new();
    if let Some(runs) = run_cmd {
        for run in runs{
            //cmd_str.push_str(&runs.iter().join(" \\\n"));
            cmd_str.push_str(&format!("RUN {} \n", run));
        }
    }
    cmd_str
}

fn build_copy_command_str(sources: &Vec<String>, destination_dir: &String) -> String {
    let mut cmd_str = String::new();
    for source in sources {
        cmd_str.push_str("COPY ");
        cmd_str.push_str(&format!("{} {} \n\n", source, destination_dir))
    }
    cmd_str
}

fn build_extra_copies_command_str(copies_command: &Option<Vec<CopyCommand>>) -> String {
    let mut cmd_str = String::new();
    if let Some(copies_command) = copies_command {
        for command in copies_command {
            let file_path = command.source.split("/");
            let names: Vec<&str> = file_path.collect();
            let filename = names.last().expect("File extension cannot be read");
            cmd_str.push_str("COPY ");
            cmd_str.push_str(&format!("{} {} \n", filename, command.destination))
        }
    }
    cmd_str
}

fn build_workdir_command_str(workdir_cmd: &Option<String>) -> String {
    let mut cmd_str = String::new();
    if let Some(workdir) = workdir_cmd {
        cmd_str.push_str("WORKDIR ");
        cmd_str.push_str(workdir);
    }
    cmd_str
}

fn build_expose_command_str(expose_ports: &Option<Vec<i32>>) -> String {
    let mut cmd_str = String::new();
    if let Some(ports) = expose_ports {
        if !ports.is_empty() {
            cmd_str.push_str("EXPOSE ");
            let ports_str = ports.iter().join(" ");
            cmd_str.push_str(&ports_str);
        }
    }
    cmd_str
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
        let container_destination_dir = "/usr/src/app/";

        let copy_str = build_copy_command_str(&sources, &container_destination_dir.to_string());
        let t1_str = "COPY f1.txt /usr/src/app/";
        let t2_str = "COPY other/f2.txt /usr/src/app/";

        assert_eq!(true, copy_str.contains(t1_str) && copy_str.contains(t2_str));
    }

    #[test]
    fn test_build_extra_copies_command_str() {
        let cp1 = CopyCommand {
            source: "f1.txt".to_string(),
            destination: "some/folder/".to_string(),
        };
        let cp2 = CopyCommand {
            source: "other/f2.txt".to_string(),
            destination: "some/other/folder/".to_string(),
        };
        let copies: Vec<CopyCommand> = vec![cp1, cp2];

        let copy_str = build_extra_copies_command_str(&Some(copies));

        let t1_str = "COPY f1.txt some/folder/";
        let t2_str = "COPY f2.txt some/other/folder/";

        assert_eq!(true, copy_str.contains(t1_str) && copy_str.contains(t2_str));
    }

    #[test]
    fn test_build_expose_command_str() {
        let ports: Vec<i32> = vec![8080, 80];
        let s = build_expose_command_str(&Some(ports));
        assert_eq!("EXPOSE 8080 80", s);
    }

    #[test]
    fn test_build_workdir_command_str() {
        let workdir = String::from("/usr/src/app/");

        let s = build_workdir_command_str(&Some(workdir));
        assert_eq!("WORKDIR /usr/src/app/", s);
    }

    // #[test]
    // fn test_build_run_command_str(){
    //     let runs: Vec<String>= vec!["ls -al".to_string(), "echo helloworld".to_string()];

    //     let str1 = "RUN ls -al \";
    // }
}
