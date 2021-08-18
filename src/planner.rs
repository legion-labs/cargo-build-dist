//! The planner module: depending on the commandline, and the context
//! build a full action plan that performs validation ahead of time,
//! the earlier we fail the better.

use cargo_toml::Error;
use itertools::Itertools;
use serde::__private::de::Content;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::{ops::Add, path::PathBuf};

use crate::DockerPackage;
pub trait Action {
    fn run(&self) -> Result<(), String>;
}

struct Dockerfile {
    content: String,
    path: PathBuf,
}

impl Dockerfile {
    fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let tpl_name = "Dockerfile";
        if let Ok(template) = tera::Template::new(
            tpl_name,
            None,
            include_str!("templates/Dockerfile.template"),
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
            let mut env_variables_str = String::new();
            if let Some(env_variables) = &docker_setting.env {
                if env_variables.is_empty() {
                    return Err("failed to render template file".to_string());
                }
                env_variables_str.push_str("ENV ");
                for env_variable in env_variables {
                    if env_variable.value.is_empty() || env_variable.name.is_empty() {
                        return Err("Environment name and value should both exist".to_string());
                    }
                    env_variables_str.push_str(&format!(
                        "{}={} \\\n",
                        env_variable.name, env_variable.value
                    ));
                }
            }
            context.insert("env_variable", &env_variables_str);

            // COPY command(s)
            let mut copy_commands_str = String::new();
            if let Some(copy_commands) = &docker_setting.copy {
                for copy_command in copy_commands {
                    let mut copy_command_str = "COPY".to_string();
                    copy_command_str = format!(
                        "{} {} {} \n",
                        copy_command_str, &copy_command.source, &copy_command.destination
                    );
                    copy_commands_str += &copy_command_str;
                }
            }
            context.insert("copy_cmd", &copy_commands_str);

            // RUN command(s)
            let mut run_commands_str = String::new();
            if let Some(run_commands) = &docker_setting.run {
                if !run_commands.is_empty() {
                    run_commands_str.push_str("RUN ");
                    for run_command in run_commands {
                        run_commands_str.push_str(run_command);
                        run_commands_str.push_str("\n");
                    }
                }
            }
            context.insert("run_cmd", &run_commands_str);

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
                wordir_cmd_str.push_str(workdir);
            }
            context.insert("workdir_cmd", &wordir_cmd_str);

            // EXPOSE command
            let mut expose_command_str = String::new();
            if let Some(ports) = &docker_setting.expose {
                if !ports.is_empty() {
                    expose_command_str.push_str("EXPOSE ");
                    let ports_str = ports.iter().join(",");
                    expose_command_str.push_str(&ports_str);
                }
            }
            context.insert("expose_cmd", &expose_command_str);

            // Todo: validate intention ?
            // To be discussed with others....
            // Can not infer that that binary should be use as executable.
            // if we have multiple binaries in our binaries vector, which one should be executed and on which order ?
            // We can also implement the ENTRYPOINT command, but in this case, CMD command is not relevant anymore.
            //let binaries = &docker_package.binaries;
            // ENTRYPOINT vs CMD command
            context.insert("executable", "cargo-dockerize");

            if let Ok(content) = tera.render(tpl_name, &context) {
                println!("{}", content);
                Ok(Self {
                    content,
                    path: "".into(),
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
        println!("-----------------------action for Dockerfile-----------------");
        if let Err(e) = std::fs::write(&self.path, &self.content) {
            Err(format!("failed to write docker file {}", e))
        } else {
            Ok(())
        }
    }
}

struct CopyFile {
    name: String,
    source: PathBuf,
    destination: PathBuf,
}

struct CopyFiles {
    copy_files: Vec<CopyFile>,
}

impl CopyFiles {
    fn new(docker_package: &DockerPackage, target_dir: &PathBuf) -> Result<Self, String> {
        let mut copy_files= vec![];
        for binary in &docker_package.binaries {
            let mut source = PathBuf::new();
            source.push(target_dir);
            source.push(binary);

            if !source.exists(){
                return Err(format!("file {:?} does'nt exist", source));
            }

            let mut destination = PathBuf::new();
            destination.push(target_dir);
            destination.push("docker");
            destination.push(binary);

            copy_files.push(CopyFile {
                name: binary.to_string(),
                source: source,
                destination: destination,
            });
        }
        Ok(Self { copy_files })
    }
}

impl Action for CopyFiles {
    fn run(&self) -> Result<(), String> {
        println!("-------------------------action for CopyFiles----------------------");
        for copy_file in &self.copy_files{
            if let Err(e) = fs::copy(&copy_file.source, &copy_file.destination){
                return Err(format!("failed to copy file {}", e));
            }
        }
        Ok(())
    }
}

pub fn plan_build(context: &super::Context) -> Result<Vec<Box<dyn Action>>, String> {
    // plan cargo build
    // plan files copies
    // plan Dockerfile creation:
    let mut actions: Vec<Box<dyn Action>> = vec![];

    for docker_package in &context.docker_packages {
        actions.push(Box::new(Dockerfile::new(docker_package)?));
        actions.push(Box::new(CopyFiles::new(
            docker_package,
            &context.target_dir,
        )?));
    }
    Ok(actions)
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
