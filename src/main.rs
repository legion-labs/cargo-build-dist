use std::env;

use clap::{App, AppSettings, Arg, SubCommand};

const ARG_NAME_DEBUG: &str = "debug";
const ARG_NAME_MANIFEST: &str = "manifest-path";

const SUBCOMMAND_NAME_BUILD: &str = "build";
const SUBCOMMAND_NAME_DRYRUN: &str = "dry-run";
const SUBCOMMAND_NAME_CHECK: &str = "check";
const SUBCOMMAND_NAME_DEPLOY: &str = "push";
const SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION: &str = "auto-repository";

fn main() -> Result<(), String> {
    let cargo = std::env::var("CARGO");
    if let Err(e) = &cargo {
        eprintln!("Failed to find the CARGO environment variable, it is usually set by cargo.");
        eprintln!("Make sure that cargo-dockerize has been run from cargo by having cargo-dockerize in your path");
        return Err(format!("cargo not found: {}", e));
    }
    let cargo = cargo.unwrap();
    let args: Vec<_> = env::args_os().collect();
    let matches = App::new("cargo dockerize")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Legion Labs <devs@legionlabs.com>")
        .about("Help managing Docker images containing cargo build artifacts")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name(ARG_NAME_DEBUG)
                .short("d")
                .long(ARG_NAME_DEBUG)
                .required(false)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_NAME_MANIFEST)
                .short("m")
                .long(ARG_NAME_MANIFEST)
                .takes_value(true)
                .required(false)
                .help("Path to Cargo.toml"),
        )
        .subcommand(
            SubCommand::with_name(SUBCOMMAND_NAME_BUILD)
                .about("Build docker image containing cargo build artifacts"),
        )
        .subcommand(
            SubCommand::with_name(SUBCOMMAND_NAME_DRYRUN)
                .about("Execute a dry-run of the build image"),
        )
        .subcommand(
            SubCommand::with_name(SUBCOMMAND_NAME_CHECK)
                .about("Check docker image based on cargo build artifacts"),
        )
        .subcommand(
            SubCommand::with_name(SUBCOMMAND_NAME_DEPLOY)
                .about("Deploy docker image")
                .arg(
                    Arg::with_name("auto-repository")
                        .long("auto-repository")
                        .required(false)
                        .help("Repository will be create automatically if not exists"),
                )
                .arg(
                    Arg::with_name("registry-type")
                        .long("registry-type")
                        .short("-r")
                        .takes_value(true)
                        .required(false)
                        .help("Repository will be create automatically if not exists"),
                ),
        )
        .get_matches_from(&args[0..]);

    if let Some(_path) = matches.value_of(ARG_NAME_MANIFEST) {
        if _path.trim().is_empty() {
            return Err(format!("ARG {} cannot be empty", ARG_NAME_MANIFEST));
        }
    }

    // build the context
    let context = cargo_dockerize::Context::build(
        &cargo,
        matches.is_present(ARG_NAME_DEBUG),
        matches.value_of(ARG_NAME_MANIFEST),
    )?;

    match matches.subcommand() {
        (SUBCOMMAND_NAME_BUILD, Some(_command_match)) => {
            if let Ok(actions) = cargo_dockerize::plan_build(&context) {
                cargo_dockerize::render(actions);
            }
        }
        (SUBCOMMAND_NAME_DRYRUN, Some(_command_match)) => {
            if let Ok(actions) = cargo_dockerize::plan_build(&context) {
                cargo_dockerize::dryrun_render(actions);
            }
        }
        (SUBCOMMAND_NAME_CHECK, Some(_command_match)) => {
            if let Err(e) = cargo_dockerize::check_build_dependencies(&context) {
                println!("{}", e);
            }
        }
        (SUBCOMMAND_NAME_DEPLOY, Some(command_match)) => {
            let auto_repository =
                command_match.is_present(SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION);
            if auto_repository {
                println!("The repository will be automatically create if not exists");
            } 

            if command_match.is_present("registry-type") {
                let registry_type = command_match.value_of("registry-type");
                if let Some(registry_type) = registry_type {
                    match registry_type {
                        "aws" => println!("aws"),
                        _ => println!("something else"),
                    }
                }
            }
            //cargo_dockerize::deploy_build(&context);
        }
        other_match => println!("Command {:?} doesn't exists", &other_match),
    }
    Ok(())
}
