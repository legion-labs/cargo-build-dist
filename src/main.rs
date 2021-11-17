use cargo_build_dist::{bail, Context};
use clap::{App, Arg};
use log::debug;
use std::{env, io::Write, path::PathBuf};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use cargo_build_dist::{Error, Result};

const ARG_DEBUG: &str = "debug";
const ARG_RELEASE: &str = "release";
const ARG_MANIFEST_PATH: &str = "manifest-path";
const ARG_VERBOSE: &str = "verbose";
const ARG_DRY_RUN: &str = "dry-run";

fn get_cargo_path() -> Result<PathBuf> {
    match std::env::var("CARGO") {
        Ok(cargo) => Ok(PathBuf::from(&cargo)),
        Err(e) => {
            Err(
                Error::new("`cargo` not found")
                .with_source(e)
                .with_explanation("The `CARGO` environment variable was not set: it is usually set by `cargo` itself.\nMake sure that `cargo build-dist` is run through `cargo` by putting its containing folder in your `PATH`."),
            )
        }
    }
}

fn main() {
    if let Err(e) = run() {
        let mut stderr = StandardStream::stderr(ColorChoice::Always);
        stderr
            .set_color(
                ColorSpec::new()
                    .set_fg(Some(Color::Red))
                    .set_intense(true)
                    .set_bold(true),
            )
            .unwrap();
        write!(&mut stderr, "Error").unwrap();
        stderr.reset().unwrap();

        writeln!(&mut stderr, ": {}", e.description()).unwrap();

        if let Some(source) = e.source() {
            stderr
                .set_color(
                    ColorSpec::new()
                        .set_fg(Some(Color::White))
                        .set_intense(true)
                        .set_bold(true),
                )
                .unwrap();
            write!(&mut stderr, "Caused by").unwrap();
            stderr.reset().unwrap();
            writeln!(&mut stderr, ": {}", source).unwrap();
        }

        if let Some(explanation) = e.explanation() {
            stderr
                .set_color(
                    ColorSpec::new()
                        .set_fg(Some(Color::Yellow))
                        .set_bold(true)
                        .set_intense(true),
                )
                .unwrap();
            writeln!(&mut stderr, "\n{}", explanation).unwrap();
            stderr.reset().unwrap();
        }

        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cargo = get_cargo_path()?;

    let matches = App::new("cargo build-dist")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Legion Labs <devs@legionlabs.com>")
        .about("Build distributable artifacts from cargo crates.")
        .arg(
            Arg::with_name(ARG_DEBUG)
                .short("d")
                .long(ARG_DEBUG)
                .required(false)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_RELEASE)
                .long(ARG_RELEASE)
                .required(false)
                .help("Use release build artifacts"),
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
                .long(ARG_DRY_RUN)
                .required(false)
                .help("Do not really push any artifacts"),
        )
        .arg(
            Arg::with_name(ARG_MANIFEST_PATH)
                .short("m")
                .long(ARG_MANIFEST_PATH)
                .takes_value(true)
                .required(false)
                .help("Path to Cargo.toml"),
        )
        .get_matches();

    let mut log_level = log::LevelFilter::Info;

    if matches.is_present(ARG_DEBUG) {
        log_level = log::LevelFilter::Debug;
    }

    env_logger::Builder::new().filter_level(log_level).init();

    if let Some(_path) = matches.value_of(ARG_MANIFEST_PATH) {
        if _path.trim().is_empty() {
            bail!("`--{}` cannot be empty", ARG_MANIFEST_PATH);
        }
    }

    debug!("Using `cargo` at: {}", cargo.display());

    let is_release = matches.is_present(ARG_RELEASE);

    if is_release {
        debug!(
            "`--{}` was specified: using release build artifacts",
            ARG_RELEASE
        );
    } else {
        debug!(
            "`--{}` was not specified: using debug build artifacts",
            ARG_RELEASE
        );
    }

    let manifest_path = matches.value_of(ARG_MANIFEST_PATH).map(PathBuf::from);

    match &manifest_path {
        Some(manifest_path) => {
            debug!(
                "`--{}` was specified: using manifest path: {}",
                ARG_MANIFEST_PATH,
                manifest_path.display()
            );
        }
        None => {
            debug!(
                "`--{}` was not specified: using current directory",
                ARG_MANIFEST_PATH
            );
        }
    }

    // build the context
    let context = Context::build(&cargo, is_release, manifest_path)
        .map_err(|e| Error::new("could not build context").with_source(e))?;

    Ok(())
}
