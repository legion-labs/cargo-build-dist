use std::fmt::Display;

use crate::{aws_lambda::AwsLambdaDistTarget, docker::DockerDistTarget, Result};

// Quite frankly, this structure is not used much and never in a context where
// its performance is critical. So we don't really care about the size of the
// enum.
#[allow(clippy::large_enum_variant)]
pub(crate) enum DistTarget<'g> {
    AwsLambda(AwsLambdaDistTarget<'g>),
    Docker(DockerDistTarget<'g>),
}

impl DistTarget<'_> {
    pub fn build(&self) -> Result<()> {
        match self {
            DistTarget::AwsLambda(dist_target) => dist_target.build(),
            DistTarget::Docker(dist_target) => dist_target.build(),
        }
    }

    pub fn publish(&self) -> Result<()> {
        match self {
            DistTarget::AwsLambda(dist_target) => dist_target.publish(),
            DistTarget::Docker(dist_target) => dist_target.publish(),
        }
    }
}

impl Display for DistTarget<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistTarget::AwsLambda(dist_target) => dist_target.fmt(f),
            DistTarget::Docker(dist_target) => dist_target.fmt(f),
        }
    }
}
