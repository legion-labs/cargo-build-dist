use std::{fmt::Display, path::PathBuf, process::Command};

use log::debug;

use crate::{dist_target::DistTarget, Dependencies, Error, ErrorContext, Result};

use super::DockerMetadata;

#[derive(Debug)]
pub struct DockerPackage {
    pub name: String,
    pub version: String,
    pub toml_path: String,
    pub binaries: Vec<String>,
    pub metadata: DockerMetadata,
    pub dependencies: Dependencies,
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

        self.build_dockerfile(dockerfile)
    }
}

impl DockerPackage {
    fn build_dockerfile(&self, docker_file: PathBuf) -> Result<()> {
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

        cmd.status().map_err(Error::from_source).with_full_context(
            "failed to build Docker image",
            "The build of the Docker image failed which could indicate a configuration problem.",
        )?;

        Ok(())
    }

    fn docker_image_name(&self) -> String {
        format!("{}:{}", self.package.name, self.package.version)
    }

    fn copy_binaries(&self) -> Result<()> {
        debug!("Will now copy all dependant binaries");

        for binary in &self.binaries {
            let source = self.target_dir.join(binary);
            let target = self.docker_root.join(binary);

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

    fn copy_extra_files(&self) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy in self.metadata.extra_copies.iter().flatten() {
            let source = self.target_dir.join(&copy.source);
            // Change the target to the Docker root instead. And make sure the generated Dockerfile has the correct path.
            todo!();
            let target = self.docker_root.join(&copy.destination);

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
        "The build process needed to create `{}` but it could not. You may want to verify permissions.",
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
                binary, &self.metadata.target_bin_dir
            ));
        }

        for copy in self.metadata.extra_copies.iter().flatten() {
            let file_name = copy.source.file_name().ok_or_else(|| Error::new("invalid copy command").with_explanation(format!("Could not determine filename in COPY command `{}`. Please verify your configuration.", copy.to_string())))?;
            content.push_str(&format!(
                "COPY {} {}\n",
                file_name.to_string_lossy(),
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
            content.push_str(&format!("WORKDIR {}\n", workdir));
        }

        content.push_str(&format!("CMD [\"./{}\"]", &self.binaries[0]));

        Ok(content)
    }
}
