use serde::Deserialize;

use crate::{
    aws_lambda::AwsLambdaDistTarget, dist_target::DistTarget, metadata::CopyCommand, Package,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsLambdaMetadata {
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub s3_bucket_prefix: String,
    #[serde(default = "default_target_runtime")]
    pub target_runtime: String,
    #[serde(default)]
    pub extra_files: Vec<CopyCommand>,
}

fn default_target_runtime() -> String {
    "x86_64-unknown-linux-musl".to_string()
}

impl AwsLambdaMetadata {
    pub(crate) fn into_dist_target<'g>(
        self,
        name: String,
        package: &'g Package<'g>,
    ) -> DistTarget<'g> {
        DistTarget::AwsLambda(AwsLambdaDistTarget {
            name,
            package,
            metadata: self,
        })
    }
}
