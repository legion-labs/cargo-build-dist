use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use cargo::{
    core::compiler::{CompileMode, CompileTarget},
    ops::{compile, CompileOptions},
};
use log::debug;

use crate::{
    dist_target::{BuildResult, DistTarget},
    Error, ErrorContext, Mode, Result,
};

use super::AwsLambdaMetadata;

#[derive(Debug)]
pub struct AwsLambdaPackage {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub binary: String,
    pub metadata: AwsLambdaMetadata,
    pub target_dir: PathBuf,
    pub lambda_root: PathBuf,
    pub mode: Mode,
    pub package: cargo_metadata::Package,
}

impl Display for AwsLambdaPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "aws-lambda[{} {}]",
            self.package.name, self.package.version
        )
    }
}

impl DistTarget for AwsLambdaPackage {
    fn package(&self) -> &cargo_metadata::Package {
        &self.package
    }

    fn build(&self, _options: &crate::BuildOptions) -> Result<BuildResult> {
        if cfg!(windows) {
            return Ok(BuildResult::Ignored(
                "AWS Lambda build is not supported on Windows".to_string(),
            ));
        }

        let binary = self.build_binary()?;
        self.copy_binary(binary)?;
        self.copy_extra_files()?;

        Ok(BuildResult::Success)
    }
}

impl AwsLambdaPackage {
    fn build_binary(&self) -> Result<PathBuf> {
        let config = cargo::util::config::Config::default().unwrap();

        let ws =
            cargo::core::Workspace::new(std::path::Path::new(&self.package.manifest_path), &config)
                .expect("Cannot create workspace");

        let mut compile_options = CompileOptions::new(&config, CompileMode::Build).unwrap();

        compile_options.spec = cargo::ops::Packages::Packages(vec![self.package.name.clone()]);
        compile_options.build_config.requested_profile =
            cargo::util::interning::InternedString::new(&self.mode.to_string());
        compile_options.build_config.requested_kinds =
            vec![cargo::core::compiler::CompileKind::Target(
                CompileTarget::new(&self.metadata.target_runtime).unwrap(),
            )];

        compile(&ws, &compile_options)
            .map(|compilation| compilation.binaries[0].path.clone())
            .map_err(|err| Error::new("failed to compile AWS Lambda binary").with_source(err))
    }

    fn copy_binary(&self, source: PathBuf) -> Result<()> {
        debug!("Will now copy the dependant binary");

        std::fs::create_dir_all(&self.lambda_root)
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `lambda_root` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", &self.lambda_root.display()),
            )?;

        // The name of the target binary is fixed to "bootstrap" by the folks at AWS.
        let target = self.lambda_root.join("bootstrap");

        debug!("Copying {} to {}", source.display(), target.display());

        std::fs::copy(source, target)
            .map_err(Error::from_source)
            .with_full_context(
                "failed to copy binary",
                format!(
                    "The binary `{}` could not be copied to the Docker image. Has this target been built before attempting its packaging?",
                    self.binary
                ),
            )?;

        Ok(())
    }

    fn package_root(&self) -> &Path {
        self.toml_path.parent().unwrap()
    }

    fn copy_extra_files(&self) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy_command in self.metadata.extra_files.iter() {
            copy_command.copy_files(&self.package_root(), &self.lambda_root)?;
        }

        Ok(())
    }
}
