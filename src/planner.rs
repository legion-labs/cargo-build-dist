//! The planner module: depending on the commandline, and the context
//! build a full action plan that performs validation ahead of time,
//! the earlier we fail the better.

use itertools::Itertools;
use std::{ops::Add, path::PathBuf};
use std::fs;

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
            context.insert("base", "ubuntu:20.04");

            //let binaries = &docker_package.binaries;
            
            // based on the dockersettings, we need to integrate the necessary docker commands
            // into the dockerfile.
            let docker_setting = &docker_package.docker_settings;           
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

            let mut expose_command_str = String::new();
            if let Some(ports) = &docker_setting.expose {
                if !ports.is_empty() {
                    expose_command_str.push_str("EXPOSE ");
                    let ports_str = ports.iter().join(",");
                    expose_command_str.push_str(&ports_str);
                }
            }

            let mut wordir_cmd_str = String::new();
            if let Some(workdir)=  &docker_setting.workdir {
                wordir_cmd_str.push_str(workdir);
            }

            let mut user_cmd_str = String::new();
            if let Some(user)=  &docker_setting.user {
                user_cmd_str.push_str(user);
            }


            context.insert("copy_cmd", &copy_commands_str);
            context.insert("run_cmd", &run_commands_str);
            context.insert("expose", &expose_command_str);
            context.insert("executable", "cargo-dockerize");
            context.insert("user",&user_cmd_str);
            context.insert("workdir", &wordir_cmd_str);

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
        if let Err(e) = std::fs::write(&self.path, &self.content) {
            Err(format!("failed to write docker file {}", e))
        } else {
            Ok(())
        }
    }
}

struct CopyBinaryFile {
    source_path: PathBuf,
    destination_path: PathBuf
}

impl CopyBinaryFile{
    fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        if 1==1 {
            Ok(Self {
                source_path:"".into(),
                destination_path:"".into(),
            })
        } else{
            Err("failed to copy binary file".to_string())
        }
    }
}

impl Action for CopyBinaryFile {
    fn run(&self) -> Result<(), String> {
        if let Err(e) = fs::copy(&self.source_path, &self.destination_path) {
            Err(format!("failed to copy binary file {}", e))
        } else {
            Ok(())
        }
    }
}

pub fn plan_build(context: &super::Context, debug: bool) -> Result<Vec<Box<dyn Action>>, String> {
    // plan cargo build
    // plan files copies
    // plan Dockerfile creation:
    let mut actions: Vec<Box<dyn Action>> = vec![];
    for docker_package in &context.docker_packages {
        actions.push(Box::new(Dockerfile::new(docker_package)?));
        actions.push(Box::new(CopyBinaryFile::new(docker_package)?));
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
