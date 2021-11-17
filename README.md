# Cargo build-dist

Builds distributable artifacts from cargo crates in various forms.

## About

`cargo build-dist` is a tool used to build various distributable artifacts to
help the distribution of cargo-generated binaries.

In its current form, it supports both build and uploading AWS Lambda packages as
well as a subset of Docker images but will likely be extended to other targets
in the future.

In addition to building packages, `cargo build-dist` also considers crate
dependencies to detect version bumps. It proves especially useful when working
on mono-repos, like the one Legion Labs maintains.

## How to

```bash
USAGE:
    cargo build-dist [FLAGS] [OPTIONS] [SUBCOMMAND]

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

## Bundle manifest format

There are several fields in the `[package.metadata.docker]` section. 

### Settings

* `base`: Every dockerfile stars with an image, the base, is the equivalent to the FROM instruction at the beginning of the dockerfile. 
* `deps_hash`: Representing the hash of the dependencies of the package.  This value is calculate based on the name and version of cumulated dependencies.  The usage of the subcommand `check` of the dockerize tool will provide the package hash dependencies.
* `env`: Set the environment variables in the container.
* `copy_dest_dir`: Specific path in the container where will be located the crate's binary.
* `extra_copies`: List of files that needed to be copieds from the a source to a specific path in the container.
* `extra_commands`: List of DOCKER command to be executed in the container.
* `expose`: List of ports on which the container will listen on.
* `workdir`: Path where the shell will be changed into.

Settings such as `base`, `copy_dest_dir`, `extra_copies`, `extra_commands`, `expose`, `workdir` are meant to construct the Dockerfile that willl be used to build a docker image.

The setting `deps_hash` is used validate that after calculating the crate's transitive dependencies (name, version), we need or not to bump the version of the crate.

Example of Cargo manifest:

```toml
...
[package.metadata.docker]

base = "ubuntu:20.04"

deps_hash = "f9d194bf3f00f0b1bbd71a6c4d4853c67229a115460aea78cc7f0dcbb80abcd2"

env = [
    { name = "TZ", value = "Etc/UTC" },
    { name = "APP_USER", value = "appuser" },
    { name = "APP", value = "/usr/src/app" },
]

copy_dest_dir = "/usr/src/app/"

extra_copies = [
    { source = "src/test/test-file", destination = "/usr/src/app/" }
]

extra_commands = [
    "RUN ls -al",
    "RUN echo hello > hello.txt",
    "RUN cat /usr/scr/app/testfile"
]

expose = [80, 100]

workdir = "/usr/src/app/"
```

Which will generate a Dockerfile with the following content:

```bash
FROM ubuntu:20.04

ENV TZ=Etc/UTC \
APP_USER=appuser \
APP=/usr/src/app

COPY cargo-dockerize /usr/src/app/
COPY test-file /usr/src/app/

RUN ls -al
RUN echo hello > hello.txt
RUN cat /usr/src/app/test-file

EXPOSE 80 100

WORKDIR /usr/src/app/

CMD ["./cargo-dockerize"]
```
