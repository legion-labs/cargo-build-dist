use crate::{Action, DockerPackage, EnvironmentVariable};
use itertools::Itertools;
use std::{path::PathBuf, process::Command};

pub struct Dockerfile {
    content: String,
    path: PathBuf,
}

impl Dockerfile {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let tpl_name = "Dockerfile";
        if let Ok(template) = tera::Template::new(
            tpl_name,
            None,
            include_str!("../templates/Dockerfile.template"),
        ) {
            let mut tera = tera::Tera::default();
            tera.set_escape_fn(escape_docker);
            tera.autoescape_on(vec!["Dockerfile"]);
            tera.templates.insert(tpl_name.to_string(), template);

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

            // COPY command(s)
            if let Ok(_str) =
                build_copy_command_str(&docker_package.binaries, &docker_setting.copy_dest_dir)
            {
                context.insert("copy_cmd", &_str);
            }

            // RUN command(s)
            if let Ok(_str) = build_run_command_str(&docker_setting.run) {
                context.insert("run_cmd", &_str);
            }

            // ADD USER command
            let mut user_cmd_str = String::new();
            if let Some(user) = &docker_setting.user {
                user_cmd_str.push_str("USER ");
                user_cmd_str.push_str(user);
            }
            context.insert("user_cmd", &user_cmd_str);

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

            if let Ok(content) = tera.render(tpl_name, &context) {
                let mut docker_file_path = PathBuf::new();
                docker_file_path.push(docker_package.target_dir.docker_dir.clone());
                docker_file_path.push(tpl_name.to_string());

                Ok(Self {
                    content,
                    path: docker_file_path,
                })
            } else {
                Err("failed to render template file".to_string())
            }
        } else {
            Err("failed to parse template file".to_string())
        }
    }
}

impl Action for Dockerfile {
    fn run(&self) -> Result<(), String> {
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(docker_dir) {
                    return Err(format!("failed to write docker file {}", e));
                }
            }
        }

        match std::fs::write(&self.path, &self.content) {
            Err(e) => return Err(format!("failed to write docker file {}", e)),
            _ => (),
        }
        Ok(())
    }

    fn dryrun(&self) -> Result<(), String> {
        if let Some(docker_dir) = self.path.parent() {
            if !docker_dir.exists() {
                println!("Create directory {}", &docker_dir.display());
            }
        }

        println!("Write the file {}", &self.path.display());
        println!("{}", &self.content);
        Ok(())
    }
}

pub struct BuildDockerImage {
    name: String,
    tag: String,
    dockerfile_path: PathBuf,
}

impl BuildDockerImage {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let dockerfile_path = PathBuf::from(&docker_package.target_dir.docker_dir);
        Ok(Self {
            name: docker_package.name.clone(),
            tag: docker_package.version.clone(),
            dockerfile_path: dockerfile_path,
        })
    }
}

impl Action for BuildDockerImage {
    fn run(&self) -> Result<(), String> {
        let docker_build = Command::new("docker")
            .current_dir(&self.dockerfile_path)
            .arg("build")
            .arg("-t")
            .arg(format!("{}:{}", &self.name, &self.tag))
            .arg(".")
            .status()
            .expect("Failed to execute docker command");
        if !docker_build.success(){
            return Err(format!("Problem to build docker image"));
        }
        Ok(())
    }

    // implement the dry run
    fn dryrun(&self) -> Result<(), String> {
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

fn build_run_command_str(run_cmd: &Option<Vec<String>>) -> Result<String, String> {
    let mut run_command_str = String::new();
    if let Some(runs) = run_cmd {
        run_command_str.push_str("RUN ");
        run_command_str.push_str(&runs.iter().join(" \\\n"));
    }
    Ok(run_command_str)
}

fn build_copy_command_str(binaries: &Vec<String>, dest_dir: &String) -> Result<String, String> {
    let mut copy_binaries_command_str = String::new();
    if binaries.is_empty() {
        return Err("failed binaries is empty".to_string());
    } else {
        for binary in binaries {
            copy_binaries_command_str.push_str("COPY ");
            copy_binaries_command_str.push_str(&format!("{} {} \n\n", binary, dest_dir))
        }
    }
    Ok(copy_binaries_command_str)
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
