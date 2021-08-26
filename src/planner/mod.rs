//! The planner module: depending on the commandline, and the context
//! build a full action plan that performs validation ahead of time,
//! the earlier we fail the better.

//use cargo_toml::Error;
//use itertools::Itertools;
//use serde::__private::de::Content;
//use std::fs::{self, create_dir};
//use std::os::unix::prelude::CommandExt;
//use std::path::Path;
//use std::process::Command;

//use std::str::FromStr;
//use std::{ops::Add, path::PathBuf};

//use crate::{CopyCommand, DockerPackage, EnvironmentVariable, TargetDir};

mod docker;
use docker::*;

mod copy;
use copy::*;

pub trait Action {
    fn run(&self) -> Result<(), String>;
    fn dryrun(&self) -> Result<(), String>;
}


pub fn plan_build(context: &super::Context) -> Result<Vec<Box<dyn Action>>, String> {
    let mut actions: Vec<Box<dyn Action>> = vec![];

    for docker_package in &context.docker_packages {
        actions.push(Box::new(Dockerfile::new(docker_package)?));
        actions.push(Box::new(CopyFiles::new(docker_package)?));
        actions.push(Box::new(BuildDockerImage::new(docker_package)?))
    }
    Ok(actions)
}

