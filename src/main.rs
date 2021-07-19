use std::env;

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
    let matches = App::new("Carog Dockerize")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Legion Labs <devs@legionlabs.com>")
        .about("Help managing Docker images containing cargo build artifacts")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("build")
                .about("build docker images")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .required(false)
                        .help("print debug information verbosely"),
                ),
        )
        .subcommand(
            SubCommand::with_name("check")
                .about("build docker images")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .required(false)
                        .help("print debug information verbosely"),
                ),
        )
        .get_matches_from(&args[1..]);

    let context = cargo_dockerize::Context::build(&cargo)?;

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    if let Some(matches) = matches.subcommand_matches("build") {
        if matches.is_present("debug") {
            // do cargo build --debug
            cargo_dockerize::plan_build(&context, true);
            cargo_dockerize::render();
        } else {
            // do cargo build --release
            cargo_dockerize::plan_build(&context, true);
            cargo_dockerize::render();
        }
    }

    Ok(())
}
