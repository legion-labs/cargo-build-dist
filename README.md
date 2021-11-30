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
cargo build-dist 0.1.0
Legion Labs <devs@legionlabs.com>
Build distributable artifacts from cargo crates.

USAGE:
    cargo-build-dist [FLAGS] [OPTIONS]

FLAGS:
    -d, --debug      Print debug information verbosely
    -n, --dry-run    Do not really push any artifacts
    -f, --force      Push artifacts even if they already exist - this can be dangerous
    -h, --help       Prints help information
        --release    Use release build artifacts
    -V, --version    Prints version information
    -v, --verbose    Print debug information verbosely

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

### Dependency check

`cargo build-dist` will check the dependencies of the crate to detect version bumps.

If a dependency hash is specified in the manifest, it will be checked against
the current dependency hash. In case of mismatch, `cargo build-dist` will abort
its execution and let you know that a version bump may be necessary. To solve
the conflict, simply update the dependency hash with the one given by `cargo
build-dist`.

```toml
[package.metadata.build-dist]
deps_hash = "68e0fa4ba2903f04582cedb135190f6448a36553cb5065cd7031be549b7ca53c"
```

### AWS Lambda

```toml
[package.metadata.build-dist.simple-lambda]
type = "aws-lambda"
s3_bucket = "some-s3-bucket" # Required. The AWS S3 bucket to upload the package to. If empty, the value of the `CARGO_BUILD_DIST_AWS_LAMBDA_S3_BUCKET` environment variable will be used.
s3_bucket_prefix = "some/prefix/" # Optional. A prefix to use in the S3 bucket in front of the generated artifacts.
region = "ca-central-1" # Optional. The AWS region to use. Defaults to the region of the AWS CLI.
binary = "my-binary" # Optional. The name of the binary to package for this lambda. Required only if the crate contains more than one binary.
extra_files = [ # A list of extra files to copy into the Docker image.
    { source = "src/test/*", destination = "/usr/src/app/" }
]
```

This will package an AWS Lambda and push it to the specified S3 bucket.

### Docker

```toml
[package.metadata.build-dist.your-image-name]
type = "docker"
registry = "1234.dkr.ecr.ca-central-1.amazonaws.com" # Required. The registy to push the image to. If empty, the value of the `CARGO_BUILD_DIST_DOCKER_REGISTRY` environment variable will be used.
target_runtime="x86_64-unknown-linux-gnu" # Optional, defaults to "x86_64-unknown-linux-gnu". The target runtime for the generated binaries. You probably don't need to change this.
allow_aws_ecr_creation = true # Optional, defaults to false. Allows the creation of AWS ECR repositories for the image.
target_bin_dir = "/usr/src/app/bin/" # Optional. The target directory in which to place the binaries. Defaults to "/bin".
template = """
FROM ubuntu:20.04
{{ copy_all_binaries }}
{{ copy_all_extra_files }}
CMD [{{ binaries.0 }}]
"""
extra_files = [ # A list of extra files to copy into the Docker image.
    { source = "src/test/*", destination = "/usr/src/app/" }
]
```

Which will generate a Dockerfile with the following content:

```bash
FROM ubuntu:20.04
ADD /usr/src/app/bin/simple /usr/src/app/bin/simple
ADD /usr/src/app/ /usr/src/app/
CMD [/usr/src/app/bin/simple]
```

This image will have the image name:
`1234.dkr.ecr.ca-central-1.amazonaws.com/your-image-name` and your current crate
version.

#### Note on AWS ECR registries

If the registry is hosted on ECR, the tool will detect it automatically (based
on the naming convention for ECR registries) and if `allow_aws_ecr_creation` is
set to `true`, it will make sure an AWS ECR repository exists for the image.

This requires that the caller has AWS credentials set up with the appropriate
permissions.