use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::Command,
};

use aws_sdk_ecr::{model::Tag, Region, SdkError};
use log::{debug, warn};
use regex::Regex;

use crate::{dist_target::DistTarget, Error, ErrorContext, Result};

use super::DockerMetadata;

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub binaries: Vec<String>,
    pub metadata: DockerMetadata,
    pub target_dir: PathBuf,
    pub docker_root: PathBuf,
    pub package: cargo_metadata::Package,
}

impl Display for DockerPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "docker[{} {}]", self.package.name, self.package.version)
    }
}

impl DistTarget for DockerPackage {
    fn package(&self) -> &cargo_metadata::Package {
        &self.package
    }

    fn build(&self, options: &crate::BuildOptions) -> Result<()> {
        let dockerfile = self.write_dockerfile()?;
        self.copy_binaries()?;
        self.copy_extra_files()?;

        self.build_dockerfile(dockerfile, options.verbose)?;
        self.push_docker_image(options.verbose, options.dry_run)
    }
}

impl DockerPackage {
    fn push_docker_image(&self, verbose: bool, dry_run: bool) -> Result<()> {
        let mut cmd = Command::new("docker");
        let docker_image_name = self.docker_image_name();

        debug!("Will now push docker image {}", docker_image_name);

        let aws_ecr_information = self.get_aws_ecr_information();

        if let Some(aws_ecr_information) = aws_ecr_information {
            debug!("AWS ECR information found: assuming the image is hosted on AWS ECR in account `{}` and region `{}`", aws_ecr_information.account_id, aws_ecr_information.region);

            if self.metadata.allow_aws_ecr_creation {
                debug!("AWS ECR repository creation is allowed for this target");

                if dry_run {
                    warn!(
                        "`--dry-run` specified, will not really ensure the ECR repository exists"
                    );
                } else {
                    self.ensure_aws_ecr_repository_exists(&aws_ecr_information)?;
                }
            } else {
                debug!("AWS ECR repository creation is not allowed for this target - if this is not intended, specify `allows_aws_ecr_creation` in `Cargo.toml`");
            }
        } else {
            debug!(
                "No AWS ECR information found - assuming the image is hosted on another provider"
            );
        }

        let args = vec!["push", &docker_image_name];

        if dry_run {
            warn!("Would now execute: docker {}", args.join(" "));
            warn!("`--dry-run` specified: not continuing for real");

            return Ok(());
        }

        debug!("Will now execute: docker {}", args.join(" "));

        cmd.args(args);

        if verbose {
            let status = cmd.status().map_err(Error::from_source).with_full_context(
                "failed to push Docker image",
                "The push of the Docker image failed which could indicate a configuration problem.",
            )?;

            if !status.success() {
                return Err(Error::new("failed to push Docker image").with_explanation(
                    "The push of the Docker image failed. Check the logs above to determine the cause.",
                ));
            }
        } else {
            let output = cmd.output().map_err(Error::from_source).with_full_context(
                "failed to push Docker image",
                "The push of the Docker image failed which could indicate a configuration problem. You may want to re-run the command with `--verbose` to get more information.",
            )?;

            if !output.status.success() {
                return Err(Error::new("failed to push Docker image")
                    .with_explanation("The push of the Docker image failed. Check the logs below to determine the cause.")
                    .with_output(String::from_utf8_lossy(&output.stderr)));
            };
        }

        Ok(())
    }

    fn ensure_aws_ecr_repository_exists(
        &self,
        aws_ecr_information: &AwsEcrInformation,
    ) -> Result<()> {
        debug!(
            "Ensuring AWS ECR repository exists for `{}`",
            aws_ecr_information.to_string()
        );

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let region_provider = Region::new(aws_ecr_information.region.clone());
            let shared_config = aws_config::from_env().region(region_provider).load().await;
            let client = aws_sdk_ecr::Client::new(&shared_config);
            let output = client
                .create_repository()
                .repository_name(&aws_ecr_information.repository_name)
                .tags(
                    Tag::builder()
                        .key("CreatedBy")
                        .value("cargo-build-dist")
                        .build(),
                )
                .tags(
                    Tag::builder()
                        .key("PackageName")
                        .value(&self.package.name)
                        .build(),
                )
                .send()
                .await;

            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    if let SdkError::ServiceError { err, .. } = &err {
                        if err.is_repository_already_exists_exception() {
                            debug!("AWS ECR repository already exists: not recreating it.");
                            return Ok(());
                        }
                    }

                    return Err(Error::from_source(err)).with_full_context(
                        "failed to create AWS ECR repository",
                        format!(
                            "The creation of the AWS ECR repository `{}` failed. \
                    Please check your credentials and permissions and make \
                    sure the repository does not already exist with incompatible tags.",
                            aws_ecr_information.to_string()
                        ),
                    );
                }
            };

            if let Some(repository) = output.repository {
                debug!(
                    "AWS ECR repository `{}` created",
                    repository.repository_name.unwrap()
                );
            }

            return Ok(());
        })
    }

    fn build_dockerfile(&self, docker_file: PathBuf, verbose: bool) -> Result<()> {
        let mut cmd = Command::new("docker");
        let docker_image_name = self.docker_image_name();

        let docker_root = docker_file
            .parent()
            .ok_or_else(|| Error::new("failed to determine Docker root"))?;

        debug!("Moving to: {}", docker_root.display());

        cmd.current_dir(docker_root);

        let args = vec!["build", "-t", &docker_image_name, "."];

        debug!("Will now execute: docker {}", args.join(" "));

        cmd.args(args);

        // Disable the annoying `Use 'docker scan' to run Snyk tests` message.
        cmd.env("DOCKER_SCAN_SUGGEST", "false");

        if verbose {
            let status = cmd.status().map_err(Error::from_source).with_full_context(
                "failed to build Docker image",
                "The build of the Docker image failed which could indicate a configuration problem.",
            )?;

            if !status.success() {
                return Err(Error::new("failed to build Docker image").with_explanation(
                    "The build of the Docker image failed. Check the logs above to determine the cause.",
                ));
            }
        } else {
            let output = cmd.output().map_err(Error::from_source).with_full_context(
                "failed to build Docker image",
                "The build of the Docker image failed which could indicate a configuration problem. You may want to re-run the command with `--verbose` to get more information.",
            )?;

            if !output.status.success() {
                return Err(Error::new("failed to build Docker image")
                    .with_explanation("The build of the Docker image failed. Check the logs below to determine the cause.")
                    .with_output(String::from_utf8_lossy(&output.stderr)));
            };
        }

        Ok(())
    }

    fn docker_image_name(&self) -> String {
        format!(
            "{}/{}:{}",
            self.metadata.registry, self.package.name, self.package.version
        )
    }

    fn get_aws_ecr_information(&self) -> Option<AwsEcrInformation> {
        AwsEcrInformation::from_string(&format!("{}/{}", self.metadata.registry, self.package.name))
    }

    fn docker_target_bin_dir(&self) -> PathBuf {
        let relative_target_bin_dir = self
            .metadata
            .target_bin_dir
            .strip_prefix("/")
            .unwrap_or(&self.metadata.target_bin_dir);

        self.docker_root.join(relative_target_bin_dir)
    }

    fn copy_binaries(&self) -> Result<()> {
        debug!("Will now copy all dependant binaries");

        let docker_target_bin_dir = self.docker_target_bin_dir();

        std::fs::create_dir_all(&docker_target_bin_dir)
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `target_bin_dir` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", &docker_target_bin_dir.display()),
            )?;

        for binary in &self.binaries {
            let source = self.target_dir.join(binary);
            let target = self.docker_target_bin_dir().join(binary);

            debug!("Copying {} to {}", source.display(), target.display());

            std::fs::copy(source, target)
                .map_err(Error::from_source)
                .with_full_context(
                    "failed to copy binary",
                    format!(
                        "The binary `{}` could not be copied to the Docker image. Has this target been built before attempting its packaging?",
                        binary
                    ),
                )?;
        }

        Ok(())
    }

    fn package_root(&self) -> &Path {
        self.toml_path.parent().unwrap()
    }

    fn copy_extra_files(&self) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy in self.metadata.extra_copies.iter().flatten() {
            let source = self.package_root().join(&copy.source);
            let target = self.docker_root.join(copy.relative_source()?);
            let target_dir = target.parent().ok_or_else(|| {
                Error::new("failed to determine target directory").with_explanation(format!(
                    "The target directory could not be determined for the extra-file `{}`.",
                    copy.source.display()
                ))
            })?;

            debug!(
                "Ensuring that the target directory `{}` exists.",
                target_dir.display()
            );

            std::fs::create_dir_all(target_dir)
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `target_bin_dir` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", &target_dir.display()),
            )?;

            debug!("Copying {} to {}", source.display(), target.display());

            std::fs::copy(source, target)
                .map_err(Error::from_source)
                .with_full_context(
                    "failed to copy extra file",
                    format!(
                        "The extra file `{}` could not be copied to the Docker image.",
                        copy.source.display(),
                    ),
                )?;
        }

        Ok(())
    }

    fn write_dockerfile(&self) -> Result<PathBuf> {
        let dockerfile = self.generate_dockerfile()?;
        let dockerfile_path = self.get_dockerfile_name();
        let dockerfile_root = dockerfile_path.parent();

        std::fs::create_dir_all(dockerfile_root.unwrap())
            .map_err(Error::from_source)
            .with_full_context(
        "could not create Dockerfile path",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", dockerfile_root.unwrap().display()),
            )?;

        debug!("Writing Dockerfile to: {}", dockerfile_path.display());

        std::fs::write(&dockerfile_path, dockerfile)
            .map_err(Error::from_source)
            .with_context("failed to write Dockerfile")?;

        Ok(dockerfile_path)
    }

    fn get_dockerfile_name(&self) -> PathBuf {
        self.docker_root.join("Dockerfile")
    }

    fn generate_dockerfile(&self) -> Result<String> {
        let mut content = format!("FROM {}\n", &self.metadata.base);

        for env in self.metadata.env.iter().flatten() {
            content.push_str(&format!("ENV {}={}\n", env.name, env.value));
        }

        for binary in &self.binaries {
            content.push_str(&format!(
                "COPY {} {}\n",
                self.metadata
                    .target_bin_dir
                    .strip_prefix("/")
                    .unwrap_or(&self.metadata.target_bin_dir)
                    .join(binary)
                    .display(),
                &self.metadata.target_bin_dir.display()
            ));
        }

        for copy in self.metadata.extra_copies.iter().flatten() {
            let relative_source = copy.relative_source()?;

            content.push_str(&format!(
                "COPY {} {}\n",
                relative_source.display(),
                copy.destination.display(),
            ));
        }

        for command in self.metadata.extra_commands.iter().flatten() {
            content.push_str(&format!("{}\n", command));
        }

        let ports: Vec<_> = self
            .metadata
            .expose
            .iter()
            .flatten()
            .map(|port| port.to_string())
            .collect();

        if ports.len() > 0 {
            content.push_str(&format!("EXPOSE {}\n", ports.join(" ")));
        }

        if let Some(workdir) = &self.metadata.workdir {
            content.push_str(&format!("WORKDIR {}\n", workdir.display()));
        }

        content.push_str(&format!("CMD [\"./{}\"]", &self.binaries[0]));

        Ok(content)
    }
}

struct AwsEcrInformation {
    pub account_id: String,
    pub region: String,
    pub repository_name: String,
}

impl AwsEcrInformation {
    pub fn from_string(input: &str) -> Option<Self> {
        let re =
            Regex::new(r"^(\d+)\.dkr\.ecr\.([a-z0-9-]+).amazonaws.com/([a-zA-Z0-9-_/]+)$").unwrap();

        let capture = re.captures_iter(input).next();

        match capture {
            Some(capture) => Some(AwsEcrInformation {
                account_id: capture[1].to_string(),
                region: capture[2].to_string(),
                repository_name: capture[3].to_string(),
            }),
            None => None,
        }
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}.dkr.ecr.{}.amazonaws.com/{}",
            self.account_id, self.region, self.repository_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_ecr_information_valid() {
        let s = "550877636976.dkr.ecr.ca-central-1.amazonaws.com/my/repo-si_tory";
        let info = AwsEcrInformation::from_string(s);

        assert!(info.is_some());
        assert_eq!(info.as_ref().unwrap().account_id, "550877636976");
        assert_eq!(info.as_ref().unwrap().region, "ca-central-1");
        assert_eq!(info.as_ref().unwrap().repository_name, "my/repo-si_tory");
        assert_eq!(info.as_ref().unwrap().to_string(), s);
    }

    #[test]
    fn test_aws_ecr_information_wrong_prefix() {
        let info =
            AwsEcrInformation::from_string("foo.550877636976.dkr.ecr.ca-central-1.amazonaws.com/");

        assert!(info.is_none());
    }

    #[test]
    fn test_aws_ecr_information_wrong_suffix() {
        let info = AwsEcrInformation::from_string(
            "550877636976.dkr.ecr.ca-central-1.amazonaws.com/foo#bar",
        );

        assert!(info.is_none());
    }
}
