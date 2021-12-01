use std::{
    fmt::Display,
    io::Write,
    path::{Path, PathBuf},
};

use aws_config::meta::region::RegionProviderChain;
use cargo::{
    core::compiler::{CompileMode, CompileTarget},
    ops::{compile, CompileOptions},
};
use log::{debug, warn};
use walkdir::WalkDir;

use crate::{
    action_step, rust::is_current_target_runtime, BuildOptions, BuildResult, DistTarget, Error,
    ErrorContext, Result,
};

use super::AwsLambdaMetadata;

#[derive(Debug)]
pub struct AwsLambdaPackage {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub binary: String,
    pub metadata: AwsLambdaMetadata,
    pub target_root: PathBuf,
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

    fn build(&self, options: &crate::BuildOptions) -> Result<BuildResult> {
        if cfg!(windows) {
            return Ok(BuildResult::Ignored(
                "AWS Lambda build is not supported on Windows".to_string(),
            ));
        }

        self.clean(options)?;

        let binary = self.build_binary(options)?;
        self.copy_binary(options, binary)?;
        self.copy_extra_files(options)?;

        let archive = self.build_zip_archive(options)?;
        self.upload_archive(archive, options)?;

        Ok(BuildResult::Success)
    }
}

impl AwsLambdaPackage {
    fn upload_archive(&self, archive: PathBuf, options: &BuildOptions) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let region = self.metadata.region.clone();

        let fut = async move {
            let region_provider =
                RegionProviderChain::first_try(region.map(aws_sdk_s3::Region::new))
                    .or_default_provider();
            let shared_config = aws_config::from_env().region(region_provider).load().await;
            let client = aws_sdk_s3::Client::new(&shared_config);

            let s3_key = format!(
                "{}{}/v{}.zip",
                &self.metadata.s3_bucket_prefix, self.package.name, self.package.version
            );

            if options.force {
                debug!("`--force` specified: not checking for the archive existence on S3 before uploading");
            } else {
                let resp = client
                    .get_object()
                    .bucket(&self.metadata.s3_bucket)
                    .key(&s3_key)
                    .send()
                    .await;

                match resp {
                    Ok(_) => {
                        debug!(
                            "AWS Lambda archive `{}` already exists in the S3 bucket `{}`: not uploading again",
                            &s3_key, &self.metadata.s3_bucket
                        );

                        action_step!(
                            "Up-to-date",
                            "AWS Lambda archive `{}` already exists in S3 bucket `{}`",
                            &s3_key,
                            &self.metadata.s3_bucket
                        );

                        return Ok(());
                    }
                    Err(err) => is_s3_no_such_key(err, &s3_key, &self.metadata.s3_bucket),
                }?;

                debug!(
                    "The AWS Lambda archive `{}` does not exist in the S3 bucket `{}`: uploading.",
                    &s3_key, &self.metadata.s3_bucket
                );
            }

            if options.dry_run {
                warn!("`--dry-run` specified, will not really upload the AWS Lambda archive to S3");
            } else {
                let data = aws_sdk_s3::ByteStream::from_path(&archive)
                    .await
                    .map_err(|err| Error::new("failed to read archive on disk").with_source(err))?;

                action_step!(
                    "Uploading",
                    "AWS Lambda archive `{}` to S3 bucket `{}`",
                    &s3_key,
                    &self.metadata.s3_bucket
                );

                client.put_object().bucket(&self.metadata.s3_bucket).key(&s3_key).body(data).send()
                .await
                .map_err(|err|
                    Error::new("failed to upload archive on S3")
                    .with_source(err)
                    .with_explanation(format!(
                        "Please check that the S3 bucket `{}` exists and that you have the correct permissions.",
                        &self.metadata.s3_bucket
                    ))
                )?;
            }

            Ok(())
        };

        runtime.block_on(fut)
    }

    fn build_zip_archive(&self, options: &BuildOptions) -> Result<PathBuf> {
        let archive_path = self.target_dir(options).join("aws-lambda.zip");

        action_step!("Packaging", "AWS Lambda archive");

        let mut archive = zip::ZipWriter::new(
            std::fs::File::create(&archive_path)
                .map_err(|err| Error::new("failed to create zip archive file").with_source(err))?,
        );

        for entry in WalkDir::new(&self.lambda_root(options)) {
            let entry = entry.map_err(|err| {
                Error::new("failed to walk lambda root directory").with_source(err)
            })?;

            let file_path = entry
                .path()
                .strip_prefix(&self.lambda_root(options))
                .map_err(|err| {
                    Error::new("failed to strip lambda root directory").with_source(err)
                })?
                .display()
                .to_string();

            let metadata = std::fs::metadata(entry.path())
                .map_err(|err| Error::new("failed to get metadata").with_source(err))?;

            let options = zip::write::FileOptions::default();

            #[cfg(not(windows))]
            let options = {
                use std::os::unix::prelude::PermissionsExt;

                options.unix_permissions(metadata.permissions().mode())
            };

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

    fn build_binary(&self, options: &BuildOptions) -> Result<PathBuf> {
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
            .map(|compilation| compilation.binaries[0].path.clone())
            .map_err(|err| Error::new("failed to compile AWS Lambda binary").with_source(err))
    }

    fn copy_binary(&self, options: &BuildOptions, source: PathBuf) -> Result<()> {
        debug!("Will now copy the dependant binary");

        let lambda_root = self.lambda_root(options);

        std::fs::create_dir_all(&self.lambda_root(options))
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `lambda_root` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", lambda_root.display()),
            )?;

        // The name of the target binary is fixed to "bootstrap" by the folks at AWS.
        let target = lambda_root.join("bootstrap");

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

    fn clean(&self, options: &BuildOptions) -> Result<()> {
        debug!("Will now clean the build directory");

        std::fs::remove_dir_all(&self.lambda_root(options)).or_else(|err| match err.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(Error::new("failed to clean the lambda root directory").with_source(err)),
        })?;

        Ok(())
    }

    fn package_root(&self) -> &Path {
        self.toml_path.parent().unwrap()
    }

    fn target_dir(&self, options: &BuildOptions) -> PathBuf {
        self.target_root
            .join(&self.metadata.target_runtime)
            .join(options.mode.to_string())
    }

    fn lambda_root(&self, options: &BuildOptions) -> PathBuf {
        self.target_dir(options)
            .join("aws-lambda")
            .join(&self.package.name)
    }

    fn copy_extra_files(&self, options: &BuildOptions) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy_command in &self.metadata.extra_files {
            copy_command.copy_files(self.package_root(), &self.lambda_root(options))?;
        }

        Ok(())
    }
}

fn is_s3_no_such_key(
    err: aws_sdk_s3::SdkError<aws_sdk_s3::error::GetObjectError>,
    s3_key: &str,
    s3_bucket: &str,
) -> Result<()> {
    match err {
        aws_sdk_s3::SdkError::ServiceError { err, .. } => {
            if !err.is_no_such_key() {
                Err(Error::from_source(err)).with_full_context(
                    "failed to check for AWS Lambda archive existence",
                    format!(
                        "Could not verify the existence of the AWS Lambda \
                                        archive `{}` in the S3 bucket `{}`. Please check \
                                        your credentials and permissions and make sure you \
                                        have the appropriate permissions.",
                        s3_key, s3_bucket
                    ),
                )
            } else {
                Ok(())
            }
        }
        _ => Err(Error::from_source(err)).with_full_context(
            "failed to check for AWS Lambda archive existence",
            format!(
                "Could not verify the existence of the AWS Lambda \
                                archive `{}` in the S3 bucket `{}`. Please check \
                                your credentials and permissions and make sure you \
                                have the appropriate permissions.",
                s3_key, s3_bucket
            ),
        ),
    }
}
