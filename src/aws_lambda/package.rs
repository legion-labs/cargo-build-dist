use std::{
    fmt::Display,
    io::Write,
    path::{Path, PathBuf},
};

use cargo::{
    core::compiler::{CompileMode, CompileTarget},
    ops::{compile, CompileOptions},
};
use log::debug;
use walkdir::WalkDir;

use crate::{
    dist_target::{BuildResult, DistTarget},
    rust::is_current_target_runtime,
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

        self.clean()?;

        let binary = self.build_binary()?;
        self.copy_binary(binary)?;
        self.copy_extra_files()?;
        self.build_zip_archive()?;

        Ok(BuildResult::Success)
    }
}

impl AwsLambdaPackage {
    fn build_zip_archive(&self) -> Result<PathBuf> {
        let archive_path = self.target_dir.join("aws-lambda.zip");
        let mut archive = zip::ZipWriter::new(
            std::fs::File::create(&archive_path)
                .map_err(|err| Error::new("failed to create zip archive file").with_source(err))?,
        );

        for entry in WalkDir::new(&self.lambda_root) {
            let entry = entry.map_err(|err| {
                Error::new("failed to walk lambda root directory").with_source(err)
            })?;

            let file_path = entry
                .path()
                .strip_prefix(&self.lambda_root)
                .map_err(|err| {
                    Error::new("failed to strip lambda root directory").with_source(err)
                })?
                .display()
                .to_string();

            let metadata = std::fs::metadata(entry.path())
                .map_err(|err| Error::new("failed to get metadata").with_source(err))?;

            let mut options = zip::write::FileOptions::default();

            if !cfg!(windows) {
                use std::os::unix::prelude::PermissionsExt;

                options = options.unix_permissions(metadata.permissions().mode());
            }

            if metadata.is_file() {
                archive.start_file(&file_path, options).map_err(|err| {
                    Error::new("failed to start writing file in the archive")
                        .with_source(err)
                        .with_output(format!("file path: {}", file_path))
                })?;

                let buf = std::fs::read(entry.path())
                    .map_err(|err| Error::new("failed to open file").with_source(err))?;

                archive.write_all(&buf).map_err(|err| {
                    Error::new("failed to write file in the archive")
                        .with_source(err)
                        .with_output(format!("file path: {}", file_path))
                })?;
            } else if metadata.is_dir() {
                archive.add_directory(&file_path, options).map_err(|err| {
                    Error::new("failed to add directory to the archive")
                        .with_source(err)
                        .with_output(format!("file path: {}", file_path))
                })?;
            }
        }

        archive
            .finish()
            .map_err(|err| Error::new("failed to write zip archive file").with_source(err))?;

        Ok(archive_path)
    }

    fn build_binary(&self) -> Result<PathBuf> {
        let config = cargo::util::config::Config::default().unwrap();

        let ws =
            cargo::core::Workspace::new(std::path::Path::new(&self.package.manifest_path), &config)
                .expect("Cannot create workspace");

        let mut compile_options = CompileOptions::new(&config, CompileMode::Build).unwrap();

        compile_options.spec = cargo::ops::Packages::Packages(vec![self.package.name.clone()]);
        compile_options.build_config.requested_profile =
            cargo::util::interning::InternedString::new(&self.mode.to_string());

        if !is_current_target_runtime(&self.metadata.target_runtime)? {
            compile_options.build_config.requested_kinds =
                vec![cargo::core::compiler::CompileKind::Target(
                    CompileTarget::new(&self.metadata.target_runtime).unwrap(),
                )];
        }

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

    fn clean(&self) -> Result<()> {
        debug!("Will now clean the build directory");

        std::fs::remove_dir_all(&self.lambda_root).map_err(|err| {
            Error::new("failed to clean the lambda root directory").with_source(err)
        })?;

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
