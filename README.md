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

## How to use it

```bash
USAGE:
    cargo build-dist [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -d, --debug      Print debug information verbosely
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Print debug information verbosely
    -n, --dry-run    Print what would be done but don't actually do it

OPTIONS:
    -m, --manifest-path <manifest-path>    Path to Cargo.toml
```

## Manifest syntax

Distribution targets can be added for any crate in the project.

There are several types of configurations available, depending on your distribution type:

### Docker

```toml
[package.metadata.build-dist.your-image-name]
type = "docker"
registry = "1234.dkr.ecr.ca-central-1.amazonaws.com"
base = "ubuntu:20.04" # The base Docker image to use.
env = [ # A list of environment variables to set.
    { name = "TZ", value = "Etc/UTC" },
    { name = "APP_USER", value = "appuser" },
    { name = "APP", value = "/usr/src/app" },
]
target_bin_dir = "/usr/src/app/" # The target directory in the Docker image to place the binary.
extra_copies = [ # A list of extra files to copy into the Docker image.
    { source = "src/test/test-file", destination = "/usr/src/app/" }
]

extra_commands = [ # A list of extra commands to run in the Docker image.
    "RUN ls -al",
    "RUN echo hello > hello.txt",
    "RUN cat /usr/scr/app/testfile"
]
expose = [80, 100] # A list of ports to expose.
workdir = "/usr/src/app/" # The working directory to run the Docker image.
```

Which will generate a Dockerfile with the following content:

```bash
FROM ubuntu:20.04
ENV TZ=Etc/UTC \
APP_USER=appuser \
APP=/usr/src/app
COPY your-crate-binary /usr/src/app/
COPY test-file /usr/src/app/
RUN ls -al
RUN echo hello > hello.txt
RUN cat /usr/src/app/test-file
EXPOSE 80 100
WORKDIR /usr/src/app/
CMD ["./your-crate-binary"]
```

This image will have the image name:
`1234.dkr.ecr.ca-central-1.amazonaws.com/your-image-name` and your current crate
version.