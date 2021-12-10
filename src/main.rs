// BEGIN - Legion Labs lints v0.6
// do not change or add/remove here, but one can add exceptions after this section
#![deny(unsafe_code)]
#![warn(future_incompatible, nonstandard_style, rust_2018_idioms)]
// Rustdoc lints
#![warn(
    rustdoc::broken_intra_doc_links,
    rustdoc::missing_crate_level_docs,
    rustdoc::private_intra_doc_links
)]
// Clippy pedantic lints, treat all as warnings by default, add exceptions in allow list
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::shadow_unrelated,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix
)]
// Clippy nursery lints, still under development
#![warn(
    clippy::debug_assert_with_mut_call,
    clippy::disallowed_method,
    clippy::disallowed_type,
    clippy::fallible_impl_from,
    clippy::imprecise_flops,
    clippy::mutex_integer,
    clippy::path_buf_push_overwrite,
    clippy::string_lit_as_bytes,
    clippy::use_self,
    clippy::useless_transmute
)]
// Clippy restriction lints, usually not considered bad, but useful in specific cases
#![warn(
    clippy::dbg_macro,
    clippy::exit,
    clippy::float_cmp_const,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::missing_enforced_import_renames,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::string_to_string,
    clippy::todo,
    clippy::unimplemented,
    clippy::verbose_file_reads
)]
// END - Legion Labs lints v0.6
// crate-specific exceptions:
#![allow()]

use cargo_monorepo::{Context, Mode, Options};
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use log::debug;
use std::{
    env,
    fmt::{Debug, Formatter},
    io::Write,
    path::PathBuf,
};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use cargo_monorepo::{Error, Result};

const ARG_DEBUG: &str = "debug";
const ARG_RELEASE: &str = "release";
const ARG_MANIFEST_PATH: &str = "manifest-path";
const ARG_VERBOSE: &str = "verbose";
const ARG_DRY_RUN: &str = "dry-run";
const ARG_FORCE: &str = "force";
const ARG_PACKAGE: &str = "package";
const ARG_PACKAGES: &str = "packages";
const ARG_CHANGED_SINCE_GIT_REF: &str = "changed-since-git-ref";

const SUB_COMMAND_HASH: &str = "hash";
const SUB_COMMAND_LIST: &str = "list";
const SUB_COMMAND_BUILD_DIST: &str = "build-dist";
const SUB_COMMAND_PUBLISH_DIST: &str = "publish-dist";
const SUB_COMMAND_EXEC: &str = "exec";
const SUB_COMMAND_TAG: &str = "tag";

struct MainError(Error);

impl Debug for MainError {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut stderr = StandardStream::stderr(ColorChoice::Always);
        writeln!(&mut stderr, "{}", self.0.description()).unwrap();

        if let Some(source) = self.0.source() {
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
            write!(&mut stderr, ": {}", source).unwrap();
        }

        if let Some(explanation) = self.0.explanation() {
            stderr
                .set_color(
                    ColorSpec::new()
                        .set_fg(Some(Color::Yellow))
                        .set_bold(true)
                        .set_intense(true),
                )
                .unwrap();
            write!(&mut stderr, "\n{}", explanation).unwrap();
            stderr.reset().unwrap();
        }

        if let Some(output) = self.0.output() {
            stderr
                .set_color(
                    ColorSpec::new()
                        .set_fg(Some(Color::Blue))
                        .set_bold(true)
                        .set_intense(true),
                )
                .unwrap();
            writeln!(&mut stderr, "\nOutput follows:").unwrap();
            stderr.reset().unwrap();
            write!(&mut stderr, "{}", output).unwrap();
        }

        Ok(())
    }
}

fn main() -> std::result::Result<(), MainError> {
    run().map_err(MainError)
}

#[allow(clippy::too_many_lines)]
fn get_matches() -> clap::ArgMatches<'static> {
    let mut args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && args[1] == "monorepo" {
        args.remove(0);
    }

    App::new("cargo monorepo")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Legion Labs <devs@legionlabs.com>")
        .about("Build distributable artifacts from cargo crates.")
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::InferSubcommands)
        .setting(AppSettings::SubcommandRequired)
        .arg(
            Arg::with_name(ARG_DEBUG)
                .short("d")
                .long(ARG_DEBUG)
                .required(false)
                .global(true)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_RELEASE)
                .long(ARG_RELEASE)
                .required(false)
                .global(true)
                .help("Use release build artifacts"),
        )
        .arg(
            Arg::with_name(ARG_VERBOSE)
                .short("v")
                .long(ARG_VERBOSE)
                .required(false)
                .global(true)
                .help("Print debug information verbosely"),
        )
        .arg(
            Arg::with_name(ARG_DRY_RUN)
                .short("n")
                .long(ARG_DRY_RUN)
                .required(false)
                .global(true)
                .help("Do not really push any artifacts"),
        )
        .arg(
            Arg::with_name(ARG_FORCE)
                .short("f")
                .long(ARG_FORCE)
                .required(false)
                .global(true)
                .help("Push artifacts even if they already exist - this can be dangerous"),
        )
        .arg(
            Arg::with_name(ARG_MANIFEST_PATH)
                .short("m")
                .long(ARG_MANIFEST_PATH)
                .takes_value(true)
                .required(false)
                .global(true)
                .help("Path to Cargo.toml"),
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_HASH)
                .arg(
                    Arg::with_name(ARG_PACKAGES)
                        .long(ARG_PACKAGES)
                        .short("p")
                        .takes_value(true)
                        .multiple(true)
                        .require_delimiter(true)
                        .conflicts_with(ARG_CHANGED_SINCE_GIT_REF)
                        .help("A list of packagse to execute the command for, separated by commas"),
                )
                .arg(
                    Arg::with_name(ARG_CHANGED_SINCE_GIT_REF)
                        .long(ARG_CHANGED_SINCE_GIT_REF)
                        .short("s")
                        .takes_value(true)
                        .conflicts_with(ARG_PACKAGES)
                        .help(
                            "Only list the packages with changes since the specified Git reference",
                        ),
                )
                .about("Print the hash of the specified package")
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_LIST)
                .arg(
                    Arg::with_name(ARG_CHANGED_SINCE_GIT_REF)
                        .long(ARG_CHANGED_SINCE_GIT_REF)
                        .short("s")
                        .takes_value(true)
                        .help(
                            "Only list the packages with changes since the specified Git reference",
                        ),
                )
                .about("List all the packages in the current workspace"),
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_BUILD_DIST)
                .about("Build the distributable artifacts for the specified package")
                .arg(
                    Arg::with_name(ARG_PACKAGES)
                        .long(ARG_PACKAGES)
                        .short("p")
                        .takes_value(true)
                        .multiple(true)
                        .require_delimiter(true)
                        .conflicts_with(ARG_CHANGED_SINCE_GIT_REF)
                        .help("A list of packagse to execute the command for, separated by commas"),
                )
                .arg(
                    Arg::with_name(ARG_CHANGED_SINCE_GIT_REF)
                        .long(ARG_CHANGED_SINCE_GIT_REF)
                        .short("s")
                        .conflicts_with(ARG_PACKAGES)
                        .takes_value(true)
                        .help(
                            "Execute the command in all the packages with changes since the specified Git reference",
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_PUBLISH_DIST)
                .about("Publish the distributable artifacts for the specified package")
                .arg(
                    Arg::with_name(ARG_PACKAGES)
                        .long(ARG_PACKAGES)
                        .short("p")
                        .takes_value(true)
                        .multiple(true)
                        .require_delimiter(true)
                        .conflicts_with(ARG_CHANGED_SINCE_GIT_REF)
                        .help("A list of packagse to execute the command for, separated by commas"),
                )
                .arg(
                    Arg::with_name(ARG_CHANGED_SINCE_GIT_REF)
                        .long(ARG_CHANGED_SINCE_GIT_REF)
                        .short("s")
                        .conflicts_with(ARG_PACKAGES)
                        .takes_value(true)
                        .help(
                            "Execute the command in all the packages with changes since the specified Git reference",
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_EXEC)
                .about("Execute a command in each of the specified packages directory or for all packages if no packages are specified")
                .arg(
                    Arg::with_name(ARG_PACKAGES)
                        .long(ARG_PACKAGES)
                        .short("p")
                        .takes_value(true)
                        .multiple(true)
                        .require_delimiter(true)
                        .conflicts_with(ARG_CHANGED_SINCE_GIT_REF)
                        .help("A list of packagse to execute the command for, separated by commas"),
                )
                .arg(
                    Arg::with_name(ARG_CHANGED_SINCE_GIT_REF)
                        .long(ARG_CHANGED_SINCE_GIT_REF)
                        .short("s")
                        .conflicts_with(ARG_PACKAGES)
                        .takes_value(true)
                        .help(
                            "Execute the command in all the packages with changes since the specified Git reference",
                        ),
                )
                .arg(
                    Arg::with_name("command")
                        .required(true)
                        .multiple(true)
                        .help(
                            "The command to execute in each package",
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name(SUB_COMMAND_TAG)
                .about("Tag the current version of the package")
                .arg(Arg::with_name(ARG_PACKAGE).help("A package to tag").required(true)),
        )
        .get_matches_from(args)
}

fn make_context(matches: &ArgMatches<'_>) -> Result<Context> {
    if let Some(path) = matches.value_of(ARG_MANIFEST_PATH) {
        if path.trim().is_empty() {
            return Err(Error::new(format!(
                "`--{}` cannot be empty",
                ARG_MANIFEST_PATH
            )));
        }
    }

    let mut context_builder = Context::builder();

    let manifest_path = matches.value_of(ARG_MANIFEST_PATH).map(PathBuf::from);

    match &manifest_path {
        Some(manifest_path) => {
            debug!(
                "`--{}` was specified: using manifest path: {}",
                ARG_MANIFEST_PATH,
                manifest_path.display()
            );

            context_builder = context_builder.with_manifest_path(manifest_path);
        }
        None => {
            debug!(
                "`--{}` was not specified: using current directory",
                ARG_MANIFEST_PATH
            );
        }
    }

    context_builder.with_options(make_options(matches)).build()
}

fn make_options(matches: &ArgMatches<'_>) -> Options {
    let mode = Mode::from_release_flag(matches.is_present(ARG_RELEASE));

    match mode {
        Mode::Debug => {
            debug!(
                "`--{}` was not specified: using debug build artifacts",
                ARG_RELEASE
            );
        }
        Mode::Release => {
            debug!(
                "`--{}` was specified: using release build artifacts",
                ARG_RELEASE
            );
        }
    }

    Options {
        dry_run: matches.is_present(ARG_DRY_RUN),
        force: matches.is_present(ARG_FORCE),
        verbose: matches.is_present(ARG_VERBOSE),
        mode,
    }
}

fn run() -> Result<()> {
    let matches = get_matches();

    let mut log_level = log::LevelFilter::Off;

    if matches.is_present(ARG_DEBUG) {
        log_level = log::LevelFilter::Debug;
    }

    env_logger::Builder::new().filter_level(log_level).init();

    debug!("Log level set to: {}", log_level);

    let context = make_context(&matches)?;

    match matches.subcommand() {
        (SUB_COMMAND_HASH, Some(sub_matches)) => {
            let packages = match sub_matches.value_of(ARG_CHANGED_SINCE_GIT_REF) {
                Some(git_ref) => context.resolve_changed_packages(git_ref)?,
                None => match sub_matches.values_of(ARG_PACKAGES) {
                    Some(packages_names) => context.resolve_packages_by_names(packages_names)?,
                    None => context.packages()?,
                },
            };

            for package in packages {
                println!("{}={}", package.name(), package.hash()?);
            }

            Ok(())
        }
        (SUB_COMMAND_LIST, Some(sub_matches)) => {
            let packages = match sub_matches.value_of(ARG_CHANGED_SINCE_GIT_REF) {
                Some(git_ref) => context.resolve_changed_packages(git_ref)?,
                None => context.packages()?,
            };

            for package in packages {
                println!("{}", package.name());
            }

            Ok(())
        }
        (SUB_COMMAND_BUILD_DIST, Some(sub_matches)) => {
            let packages = match sub_matches.value_of(ARG_CHANGED_SINCE_GIT_REF) {
                Some(git_ref) => context.resolve_changed_packages(git_ref)?,
                None => match sub_matches.values_of(ARG_PACKAGES) {
                    Some(packages_names) => context.resolve_packages_by_names(packages_names)?,
                    None => context.packages()?,
                },
            };

            for package in packages {
                package.build_dist_targets()?;
            }

            Ok(())
        }
        (SUB_COMMAND_PUBLISH_DIST, Some(sub_matches)) => {
            let packages = match sub_matches.value_of(ARG_CHANGED_SINCE_GIT_REF) {
                Some(git_ref) => context.resolve_changed_packages(git_ref)?,
                None => match sub_matches.values_of(ARG_PACKAGES) {
                    Some(packages_names) => context.resolve_packages_by_names(packages_names)?,
                    None => context.packages()?,
                },
            };

            for package in packages {
                package.publish_dist_targets()?;
            }

            Ok(())
        }
        (SUB_COMMAND_EXEC, Some(sub_matches)) => {
            let packages = match sub_matches.value_of(ARG_CHANGED_SINCE_GIT_REF) {
                Some(git_ref) => context.resolve_changed_packages(git_ref)?,
                None => match sub_matches.values_of(ARG_PACKAGES) {
                    Some(packages_names) => context.resolve_packages_by_names(packages_names)?,
                    None => context.packages()?,
                },
            };

            let args: Vec<&str> = sub_matches.values_of("command").unwrap().collect();

            for package in packages {
                package.execute(&args)?;
            }

            Ok(())
        }
        (SUB_COMMAND_TAG, Some(sub_matches)) => {
            let package_name = sub_matches.value_of(ARG_PACKAGE).unwrap();
            let package = context.resolve_package_by_name(package_name)?;

            package.tag()
        }
        (cmd, _) => Err(
            Error::new("Unknown subcommand specified").with_explanation(format!(
                "Please specify a valid subcommand: `{}` is not a valid subcommand",
                cmd,
            )),
        ),
    }
}
