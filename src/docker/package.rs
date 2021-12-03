use std::{
    collections::HashSet,
    fmt::Display,
    path::{Path, PathBuf},
    process::Command,
};

use aws_sdk_ecr::{model::Tag, Region, SdkError};
use cargo::{
    core::compiler::{CompileMode, CompileTarget},
    ops::{compile, CompileOptions},
};
use log::{debug, warn};
use regex::Regex;

use crate::{
    action_step, rust::is_current_target_runtime, BuildOptions, BuildResult, DistTarget, Error,
    ErrorContext, Result,
};

use super::DockerMetadata;

pub const DEFAULT_DOCKER_REGISTRY_ENV_VAR_NAME: &str = "CARGO_BUILD_DIST_DOCKER_REGISTRY";

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub metadata: DockerMetadata,
    pub target_root: PathBuf,
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

    fn build(&self, options: &crate::BuildOptions) -> Result<BuildResult> {
        if cfg!(windows) {
            return Ok(BuildResult::Ignored(
                "Docker build is not supported on Windows".to_string(),
            ));
        }

        self.clean(options)?;

        let binaries = self.build_binaries(options)?;
        let dockerfile = self.write_dockerfile(options, &binaries)?;
        self.copy_binaries(options, &binaries)?;
        self.copy_extra_files(options)?;

        self.build_dockerfile(options, &dockerfile)?;
        self.push_docker_image(options)?;

        Ok(BuildResult::Success)
    }
}

impl DockerPackage {
    fn pull_docker_image(docker_image_name: &str, options: &crate::BuildOptions) -> Result<bool> {
        let mut cmd = Command::new("docker");

        debug!(
            "Will now pull docker image `{}` to check for existence",
            docker_image_name
        );

        let args = vec!["pull", docker_image_name];

        action_step!("Running", "`docker {}`", args.join(" "),);

        cmd.args(args);

        if options.verbose {
            let status = cmd.status().map_err(Error::from_source).with_full_context(
                "failed to pull Docker image",
                "The pull of the Docker image failed which could indicate a configuration problem.",
            )?;

            Ok(status.success())
        } else {
            let output = cmd.output().map_err(Error::from_source).with_full_context(
                "failed to pull Docker image",
                "The pull of the Docker image failed which could indicate a configuration problem. You may want to re-run the command with `--verbose` to get more information.",
            )?;

            Ok(output.status.success())
        }
    }

    fn push_docker_image(&self, options: &crate::BuildOptions) -> Result<()> {
        let mut cmd = Command::new("docker");
        let docker_image_name = self.docker_image_name()?;

        if options.force {
            debug!("`--force` specified: not checking for Docker image existence before pushing");
        } else if Self::pull_docker_image(&docker_image_name, options)? {
            debug!(
                "Docker image `{}` already exists: not pushing unless `--force` is specified",
                docker_image_name
            );

            return Ok(());
        }

        debug!("Will now push docker image `{}`", docker_image_name);

        let aws_ecr_information = self.get_aws_ecr_information()?;

        if let Some(aws_ecr_information) = aws_ecr_information {
            debug!("AWS ECR information found: assuming the image is hosted on AWS ECR in account `{}` and region `{}`", aws_ecr_information.account_id, aws_ecr_information.region);

            if self.metadata.allow_aws_ecr_creation {
                debug!("AWS ECR repository creation is allowed for this target");

                if options.dry_run {
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

        if options.dry_run {
            warn!("Would now execute: docker {}", args.join(" "));
            warn!("`--dry-run` specified: not continuing for real");

            return Ok(());
        }

        action_step!("Running", "`docker {}`", args.join(" "),);

        cmd.args(args);

        if options.verbose {
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
                        .value("cargo-monorepo")
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

            Ok(())
        })
    }

    fn build_dockerfile(&self, options: &crate::BuildOptions, docker_file: &Path) -> Result<()> {
        let mut cmd = Command::new("docker");
        let docker_image_name = self.docker_image_name()?;

        let docker_root = docker_file
            .parent()
            .ok_or_else(|| Error::new("failed to determine Docker root"))?;

        debug!("Moving to: {}", docker_root.display());

        cmd.current_dir(docker_root);

        let args = vec!["build", "-t", &docker_image_name, "."];

        action_step!("Running", "`docker {}`", args.join(" "),);

        cmd.args(args);

        // Disable the annoying `Use 'docker scan' to run Snyk tests` message.
        cmd.env("DOCKER_SCAN_SUGGEST", "false");

        if options.verbose {
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

    fn registry(&self) -> Result<String> {
        match self.metadata.registry {
            Some(ref registry) => Ok(registry.clone()),
            None => {
                if let Ok(registry) = std::env::var(DEFAULT_DOCKER_REGISTRY_ENV_VAR_NAME) {
                    Ok(registry)
                } else {
                    Err(
                        Error::new("failed to determine Docker registry").with_explanation(
                            format!(
                        "The field registry is empty and the environment variable {} was not set",
                        DEFAULT_DOCKER_REGISTRY_ENV_VAR_NAME
                    ),
                        ),
                    )
                }
            }
        }
    }

    fn docker_image_name(&self) -> Result<String> {
        Ok(format!(
            "{}/{}:{}",
            self.registry()?,
            self.package.name,
            self.package.version
        ))
    }

    fn get_aws_ecr_information(&self) -> Result<Option<AwsEcrInformation>> {
        Ok(AwsEcrInformation::from_string(&format!(
            "{}/{}",
            self.registry()?,
            self.package.name
        )))
    }

    fn target_dir(&self, options: &BuildOptions) -> PathBuf {
        self.target_root.join(options.mode.to_string())
    }

    fn docker_root(&self, options: &BuildOptions) -> PathBuf {
        self.target_dir(options)
            .join("docker")
            .join(&self.package.name)
    }

    fn docker_target_bin_dir(&self, options: &BuildOptions) -> PathBuf {
        let relative_target_bin_dir = self
            .metadata
            .target_bin_dir
            .strip_prefix("/")
            .unwrap_or(&self.metadata.target_bin_dir);

        self.docker_root(options).join(relative_target_bin_dir)
    }

    fn build_binaries(&self, options: &BuildOptions) -> Result<Vec<PathBuf>> {
        let config = cargo::util::config::Config::default().unwrap();

        let ws =
            cargo::core::Workspace::new(std::path::Path::new(&self.package.manifest_path), &config)
                .expect("Cannot create workspace");

        let mut compile_options = CompileOptions::new(&config, CompileMode::Build).unwrap();

        compile_options.spec = cargo::ops::Packages::Packages(vec![self.package.name.clone()]);
        compile_options.build_config.requested_profile =
            cargo::util::interning::InternedString::new(&options.mode.to_string());

        if !is_current_target_runtime(&self.metadata.target_runtime)? {
            compile_options.build_config.requested_kinds =
                vec![cargo::core::compiler::CompileKind::Target(
                    CompileTarget::new(&self.metadata.target_runtime).unwrap(),
                )];
        }

        compile(&ws, &compile_options)
            .map(|compilation| {
                compilation
                    .binaries
                    .iter()
                    .map(|b| b.path.clone())
                    .collect()
            })
            .map_err(|err| Error::new("failed to compile Docker binaries").with_source(err))
    }

    fn copy_binaries(&self, options: &BuildOptions, source_binaries: &[PathBuf]) -> Result<()> {
        debug!("Will now copy all dependant binaries");

        let docker_target_bin_dir = self.docker_target_bin_dir(options);

        std::fs::create_dir_all(&docker_target_bin_dir)
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `target_bin_dir` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", &docker_target_bin_dir.display()),
            )?;

        for source in source_binaries {
            let binary = source.file_name().unwrap().to_string_lossy().to_string();
            let target = self.docker_target_bin_dir(options).join(&binary);

            debug!("Copying {} to {}", source.display(), target.display());

            std::fs::copy(source, target)
                .map_err(Error::from_source)
                .with_full_context(
                    "failed to copy binary",
                    format!(
                        "The binary `{}` could not be copied to the Docker image.",
                        binary
                    ),
                )?;
        }

        Ok(())
    }

    fn clean(&self, options: &BuildOptions) -> Result<()> {
        debug!("Will now clean the build directory");

        std::fs::remove_dir_all(&self.docker_root(options)).or_else(|err| match err.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(Error::new("failed to clean the docker root directory").with_source(err)),
        })?;

        Ok(())
    }

    fn package_root(&self) -> &Path {
        self.toml_path.parent().unwrap()
    }

    fn copy_extra_files(&self, options: &BuildOptions) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy_command in &self.metadata.extra_files {
            copy_command.copy_files(self.package_root(), &self.docker_root(options))?;
        }

        Ok(())
    }

    fn write_dockerfile(&self, options: &BuildOptions, binaries: &[PathBuf]) -> Result<PathBuf> {
        let dockerfile = self.generate_dockerfile(binaries)?;

        debug!("Generated Dockerfile:\n{}", dockerfile);

        let dockerfile_path = self.get_dockerfile_name(options);
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

    fn get_dockerfile_name(&self, options: &BuildOptions) -> PathBuf {
        self.docker_root(options).join("Dockerfile")
    }

    fn generate_context(&self, binaries: &[PathBuf]) -> tera::Context {
        let mut context = tera::Context::new();

        context.insert("package_name", &self.package.name);
        context.insert("package_version", &self.package.version);

        let binaries: Vec<String> = binaries
            .iter()
            .map(|binary| {
                self.metadata
                    .target_bin_dir
                    .join(binary.file_name().unwrap())
                    .display()
                    .to_string()
            })
            .collect();

        context.insert("binaries", &binaries);

        let extra_files: HashSet<String> = self
            .metadata
            .extra_files
            .iter()
            .map(|cc| cc.destination.display().to_string())
            .collect();

        context.insert("extra_files", &extra_files);

        // Add some helper for common patterns to improve user experience.

        let copy_all_binaries = tera::Tera::one_off(
            "
# Copy all binaries to the Docker image.
{% for binary in binaries -%}
ADD {{ binary }} {{ binary }}
{% endfor -%}
# End of copy.
",
            &context,
            false,
        )
        .unwrap();

        context.insert("copy_all_binaries", copy_all_binaries.trim());

        let copy_all_extra_files = tera::Tera::one_off(
            "
# Copy all extra files to the Docker image.
{% for extra_file in extra_files -%}
ADD {{ extra_file }} {{ extra_file }}
{% endfor -%}
# End of copy.
",
            &context,
            false,
        )
        .unwrap();

        context.insert("copy_all_extra_files", copy_all_extra_files.trim());

        context
    }

    fn generate_dockerfile(&self, binaries: &[PathBuf]) -> Result<String> {
        let context = self.generate_context(binaries);

        tera::Tera::one_off(&self.metadata.template, &context, false)
            .map_err(Error::from_source).with_full_context(
                "failed to render Dockerfile template",
                "The specified Dockerfile template could not rendered properly, which may indicate a possible syntax error."
            )
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

        let captures = re.captures_iter(input).next();

        captures.map(|captures| Self {
            account_id: captures[1].to_string(),
            region: captures[2].to_string(),
            repository_name: captures[3].to_string(),
        })
    }
}

impl Display for AwsEcrInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
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
