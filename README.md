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

| Type | Description |
|-|-|
| `aws-lambda` | AWS Lambda package. |
| `docker` | Docker image. |

The sections hereafter describe the configuration for each type.

### Docker

```toml
[package.metadata.build-dist.your-image-name]
type = "docker"
registry = "1234.dkr.ecr.ca-central-1.amazonaws.com"
target_bin_dir = "/usr/src/app/" # Optional. The target directory in which to place the binaries. Defaults to "/bin".
template = """
FROM ubuntu:20.04
{{ copy_all_binaries }}
{{ copy_all_extra_files }}
CMD [{{ binaries.0 }}]
"""
extra_copies = [ # A list of extra files to copy into the Docker image.
    { source = "src/test/test-file", destination = "/usr/src/app/" }
]
```

Which will generate a Dockerfile with the following content:

```bash
FROM ubuntu:20.04
ADD /bin/simple /bin/simple
ADD /usr/src/app/ /usr/src/app/
CMD [/bin/simple]
```

This image will have the image name:
`1234.dkr.ecr.ca-central-1.amazonaws.com/your-image-name` and your current crate
version.