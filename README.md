# cargo-dockerize subcommand

The goal of this cargo sub command is to package a given crate binaries in a docker container, it takes into consideration the crate dependencies to define if the crate version should be bumped effectively, it is more useful in the context of a monorepo like the one Legion Labs maintains.

## How to

```bash
USAGE:
    cargo-dockerize [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -d, --debug      Print debug information verbosely
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Print debug information verbosely

OPTIONS:
    -m, --manifest-path <manifest-path>    Path to Cargo.toml

SUBCOMMANDS:
    build      Build docker image containing cargo build artifacts
    check      Check docker image based on cargo build artifacts
    dry-run    Execute a dry-run of the build image
    help       Prints this message or the help of the given subcommand(s)
    push       Deploy docker image
```
