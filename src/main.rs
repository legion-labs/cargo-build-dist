use std::{env, path::PathBuf};

use clap::{App, AppSettings, Arg, SubCommand};

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
            Arg::with_name("debug")
                .short("d")
                .required(false)
                .help("print debug information verbosely"),
        )
        .arg(
            Arg::with_name("manifest-path")
                .short("m")
                .takes_value(true)
                .required(false)
                .help("Path to Cargo.toml"),
        )
        .subcommand(SubCommand::with_name("build").about("build docker images"))
        .subcommand(SubCommand::with_name("check").about("check docker images"))
        .get_matches_from(&args[1..]);

    if let Some(_path) = matches.value_of("manifest-path") {
        if _path.trim().is_empty() {
            return Err(format!("manifest-path cannot be empty"));
        }
    }
    let is_debug = matches.is_present("debug");
    let context =
        cargo_dockerize::Context::build(&cargo, is_debug, matches.value_of("manifest-path"))?;

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    if let Some(matches) = matches.subcommand_matches("build") {
        println!("Execute build");
        if let Ok(actions) = cargo_dockerize::plan_build(&context) {
            cargo_dockerize::render(actions);
        }
    }
    if let Some(matches) = matches.subcommand_matches("check") {
        println!("Execute check on");
    }

    Ok(())
}
