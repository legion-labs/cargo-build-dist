use anyhow::bail;
use cargo_build_dist::Context;
use clap::{App, AppSettings, Arg};
use std::{env, path::PathBuf};

const ARG_DEBUG: &str = "debug";
const ARG_MANIFEST: &str = "manifest-path";
const ARG_VERBOSE: &str = "verbose";
const ARG_DRY_RUN: &str = "dry-run";

fn get_cargo_path() -> anyhow::Result<PathBuf> {
    match std::env::var("CARGO") {
        Ok(cargo) => Ok(PathBuf::from(&cargo)),
        Err(e) => {
            eprintln!("Failed to find the CARGO environment variable, it is usually set by cargo.");
            eprintln!("Make sure that cargo-dockerize has been run from cargo by having cargo-dockerize in your path");

            bail!("`cargo` not found: {}", e);
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cargo = get_cargo_path()?;

    let args: Vec<_> = env::args_os().collect();
    let matches = App::new("cargo build-dist")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Legion Labs <devs@legionlabs.com>")
        .about("Build distributable artifacts from cargo crates.")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name(ARG_DEBUG)
                .short("d")
                .long(ARG_DEBUG)
                .required(false)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_VERBOSE)
                .short("v")
                .long(ARG_VERBOSE)
                .required(false)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_DRY_RUN)
                .short("n")
                .long(ARG_VERBOSE)
                .required(false)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_MANIFEST)
                .short("m")
                .long(ARG_MANIFEST)
                .takes_value(true)
                .required(false)
                .help("Path to Cargo.toml"),
        )
        .get_matches_from(&args[1..]);

    if let Some(_path) = matches.value_of(ARG_MANIFEST) {
        if _path.trim().is_empty() {
            bail!("`--{}` cannot be empty", ARG_MANIFEST);
        }
    }

    // build the context
    let context = Context::build(
        &cargo,
        matches.is_present(ARG_DEBUG),
        matches.value_of(ARG_MANIFEST),
    )?;

    Ok(())
}
