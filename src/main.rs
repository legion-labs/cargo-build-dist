use std::env;

use clap::{App, AppSettings, Arg, SubCommand};

const ARG_NAME_DEBUG: &str = "debug";
const ARG_NAME_MANIFEST: &str = "manifest-path";
const ARG_NAME_VERBOSE: &str = "verbose";

const SUBCOMMAND_NAME_BUILD: &str = "build";
const SUBCOMMAND_NAME_DRYRUN: &str = "dry-run";
const SUBCOMMAND_NAME_CHECK: &str = "check";
const SUBCOMMAND_NAME_PUSH: &str = "push";
const SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION: &str = "auto-repository";
const SUBCOMMAND_NAME_REGISTRY_TYPE: &str = "registry";

const DEFAULT_REGISTRY_TYPE: &str = "aws";
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
            Arg::with_name(ARG_NAME_VERBOSE)
                .short("v")
                .long(ARG_NAME_VERBOSE)
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
            SubCommand::with_name(SUBCOMMAND_NAME_PUSH)
                .about("Deploy docker image")
                .arg(
                    Arg::with_name(SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION)
                        .short("-a")
                        .long(SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION)
                        .required(false)
                        .help("Repository will be create automatically if not exists"),
                )
                .arg(
                    Arg::with_name(SUBCOMMAND_NAME_REGISTRY_TYPE)
                        .long(SUBCOMMAND_NAME_REGISTRY_TYPE)
                        .short("-r")
                        .takes_value(true)
                        .default_value(DEFAULT_REGISTRY_TYPE)
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
            match cargo_dockerize::plan_build(&context) {
                Ok(actions) => {
                    let result = cargo_dockerize::check_build_dependencies(&context);
                    match result {
                        Ok(()) => {
                            cargo_dockerize::render(actions, matches.is_present(ARG_NAME_VERBOSE))
                        }
                        Err(e) => println!("{}", e),
                    }
                }
                Err(e) => println!("Problem while prepare plan {}", e),
            }
        }
        (SUBCOMMAND_NAME_DRYRUN, Some(_command_match)) => {
            match cargo_dockerize::plan_build(&context) {
                Ok(actions) => {
                    let result = cargo_dockerize::check_build_dependencies(&context);
                    match result {
                        Ok(()) => cargo_dockerize::dryrun_render(actions),
                        Err(e) => println!("{}", e),
                    }
                }
                Err(e) => println!("Problem while preparing the plan {}", e),
            }
        }
        (SUBCOMMAND_NAME_CHECK, Some(_command_match)) => {
            if let Err(e) = cargo_dockerize::check_build_dependencies(&context) {
                println!("Failed to check build dependencies: {}", e);
            }
        }
        (SUBCOMMAND_NAME_PUSH, Some(command_match)) => {
            if let Err(e) = cargo_dockerize::push_builded_image(
                &context,
                command_match
                    .value_of(SUBCOMMAND_NAME_REGISTRY_TYPE)
                    .unwrap_or_default(),
                command_match.is_present(SUBCOMMAND_NAME_AUTO_REPOSITORY_CREATION),
            ) {
                println!("Failed to push builded image : {}", e);
            }
        }
        other_match => println!("Command {:?} doesn't exists", &other_match),
    }
    Ok(())
}
