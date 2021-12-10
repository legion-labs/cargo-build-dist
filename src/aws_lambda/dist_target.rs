use std::{fmt::Display, io::Write, path::PathBuf};

use aws_config::meta::region::RegionProviderChain;
use cargo::{
    core::compiler::{CompileMode, CompileTarget},
    ops::{compile, CompileOptions},
};
use log::{debug, warn};
use walkdir::WalkDir;

use crate::{
    action_step, ignore_step, rust::is_current_target_runtime, Context, Error, ErrorContext,
    Package, Result,
};

use super::AwsLambdaMetadata;

pub const DEFAULT_AWS_LAMBDA_S3_BUCKET_ENV_VAR_NAME: &str = "CARGO_MONOREPO_AWS_LAMBDA_S3_BUCKET";

pub struct AwsLambdaDistTarget<'g> {
    pub name: String,
    pub package: Package<'g>,
    pub metadata: AwsLambdaMetadata,
}

impl Display for AwsLambdaDistTarget<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "aws-lambda[{}]", self.package.name())
    }
}

impl<'g> AwsLambdaDistTarget<'g> {
    pub fn build(&self, context: &Context) -> Result<()> {
        if cfg!(windows) {
            ignore_step!(
                "Unsupported",
                "AWS Lambda build is not supported on Windows"
            );
            return Ok(());
        }

        self.clean(context)?;

        let binary = self.build_binary(context)?;
        self.copy_binary(context, binary)?;
        self.copy_extra_files(context)?;

        self.build_zip_archive(context)?;

        Ok(())
    }

    pub fn publish(&self, context: &Context) -> Result<()> {
        if cfg!(windows) {
            ignore_step!(
                "Unsupported",
                "AWS Lambda publish is not supported on Windows"
            );
            return Ok(());
        }

        if context.options().mode.is_debug() && !context.options().force {
            ignore_step!(
                "Unsupported",
                "AWS Lambda can't be published in debug mode unless `--force` is specified"
            );
            return Ok(());
        }

        self.upload_archive(context)?;

        Ok(())
    }

    fn upload_archive(&self, context: &Context) -> Result<()> {
        let archive_path = self.archive_path(context);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let region = self.metadata.region.clone();
        let s3_bucket = self.s3_bucket()?;

        let fut = async move {
            let region_provider =
                RegionProviderChain::first_try(region.map(aws_sdk_s3::Region::new))
                    .or_default_provider();
            let shared_config = aws_config::from_env().region(region_provider).load().await;
            let client = aws_sdk_s3::Client::new(&shared_config);

            let s3_key = format!(
                "{}{}/v{}.zip",
                &self.metadata.s3_bucket_prefix,
                self.package.name(),
                self.package.version()
            );

            if context.options().force {
                debug!("`--force` specified: not checking for the archive existence on S3 before uploading");
            } else {
                let resp = client
                    .get_object()
                    .bucket(&s3_bucket)
                    .key(&s3_key)
                    .send()
                    .await;

                match resp {
                    Ok(_) => {
                        debug!(
                            "AWS Lambda archive `{}` already exists in the S3 bucket `{}`: not uploading again",
                            &s3_key, &s3_bucket
                        );

                        ignore_step!(
                            "Up-to-date",
                            "AWS Lambda archive `{}` already exists in S3 bucket `{}`",
                            &s3_key,
                            &s3_bucket
                        );

                        return Ok(());
                    }
                    Err(err) => is_s3_no_such_key(err, &s3_key, &s3_bucket),
                }?;

                debug!(
                    "The AWS Lambda archive `{}` does not exist in the S3 bucket `{}`: uploading.",
                    &s3_key, &s3_bucket
                );
            }

            if context.options().dry_run {
                warn!("`--dry-run` specified, will not really upload the AWS Lambda archive to S3");
            } else {
                let data = aws_sdk_s3::ByteStream::from_path(&archive_path)
                    .await
                    .map_err(|err| Error::new("failed to read archive on disk").with_source(err))?;

                action_step!(
                    "Uploading",
                    "AWS Lambda archive `{}` to S3 bucket `{}`",
                    &s3_key,
                    &s3_bucket
                );

                client.put_object().bucket(&s3_bucket).key(&s3_key).body(data).send()
                .await
                .map_err(|err|
                    Error::new("failed to upload archive on S3")
                    .with_source(err)
                    .with_explanation(format!(
                        "Please check that the S3 bucket `{}` exists and that you have the correct permissions.",
                        &s3_bucket
                    ))
                )?;
            }

            Ok(())
        };

        runtime.block_on(fut)
    }

    fn archive_path(&self, context: &Context) -> PathBuf {
        self.target_dir(context).join("aws-lambda.zip")
    }

    fn build_zip_archive(&self, context: &Context) -> Result<()> {
        let archive_path = self.archive_path(context);

        action_step!("Packaging", "AWS Lambda archive");

        let mut archive = zip::ZipWriter::new(
            std::fs::File::create(&archive_path)
                .map_err(|err| Error::new("failed to create zip archive file").with_source(err))?,
        );

        let lambda_root = &self.lambda_root(context);

        for entry in WalkDir::new(lambda_root) {
            let entry = entry.map_err(|err| {
                Error::new("failed to walk lambda root directory").with_source(err)
            })?;

            let file_path = entry
                .path()
                .strip_prefix(lambda_root)
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

        Ok(())
    }

    fn build_binary(&self, context: &Context) -> Result<PathBuf> {
        let ws = context.workspace()?;
        let mut compile_options = CompileOptions::new(&ws.config(), CompileMode::Build).unwrap();

        compile_options.spec =
            cargo::ops::Packages::Packages(vec![self.package.name().to_string()]);
        compile_options.build_config.requested_profile =
            cargo::util::interning::InternedString::new(&context.options().mode.to_string());

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

    fn copy_binary(&self, context: &Context, source: PathBuf) -> Result<()> {
        debug!("Will now copy the dependant binary");

        let lambda_root = self.lambda_root(context);

        std::fs::create_dir_all(&lambda_root)
            .map_err(Error::from_source)
            .with_full_context(
        "could not create `lambda_root` in Docker root",
        format!("The build process needed to create `{}` but it could not. You may want to verify permissions.", lambda_root.display()),
            )?;

        // The name of the target binary is fixed to "bootstrap" by the folks at AWS.
        let target = lambda_root.join("bootstrap");

        debug!("Copying {} to {}", source.display(), target.display());

        std::fs::copy(&source, target)
            .map_err(Error::from_source)
            .with_full_context(
                "failed to copy binary",
                format!(
                    "The binary `{}` could not be copied to the Docker image. Has this target been built before attempting its packaging?",
                    source.display(),
                ),
            )?;

        Ok(())
    }

    fn clean(&self, context: &Context) -> Result<()> {
        debug!("Will now clean the build directory");

        std::fs::remove_dir_all(&self.lambda_root(context)).or_else(|err| match err.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(Error::new("failed to clean the lambda root directory").with_source(err)),
        })?;

        Ok(())
    }

    fn s3_bucket(&self) -> Result<String> {
        match &self.metadata.s3_bucket {
            Some(s3_bucket) => Ok(s3_bucket.clone()),
            None => {
                if let Ok(s3_bucket) = std::env::var(DEFAULT_AWS_LAMBDA_S3_BUCKET_ENV_VAR_NAME) {
                    Ok(s3_bucket)
                } else {
                    Err(
                        Error::new("failed to determine AWS S3 bucket").with_explanation(format!(
                        "The field s3_bucket is empty and the environment variable {} was not set",
                        DEFAULT_AWS_LAMBDA_S3_BUCKET_ENV_VAR_NAME
                    )),
                    )
                }
            }
        }
    }

    fn target_dir(&self, context: &Context) -> PathBuf {
        context
            .target_root()
            .unwrap()
            .join(&self.metadata.target_runtime)
            .join(context.options().mode.to_string())
    }

    fn lambda_root(&self, context: &Context) -> PathBuf {
        self.target_dir(context)
            .join("aws-lambda")
            .join(self.package.name())
    }

    fn copy_extra_files(&self, context: &Context) -> Result<()> {
        debug!("Will now copy all extra files");

        for copy_command in &self.metadata.extra_files {
            copy_command.copy_files(&self.package.root(), &self.lambda_root(context))?;
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
