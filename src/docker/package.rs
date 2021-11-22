use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::Command,
};

use log::debug;

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

        self.build_dockerfile(dockerfile, options.verbose)
    }
}

impl DockerPackage {
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
        format!("{}:{}", self.package.name, self.package.version)
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
