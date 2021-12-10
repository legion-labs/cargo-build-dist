use std::fmt::Display;

use crate::{aws_lambda::AwsLambdaDistTarget, docker::DockerDistTarget, Context, Result};

pub(crate) enum DistTarget<'g> {
    AwsLambda(AwsLambdaDistTarget<'g>),
    Docker(DockerDistTarget<'g>),
}

impl DistTarget<'_> {
    pub fn build(&self, context: &Context) -> Result<()> {
        match self {
            DistTarget::AwsLambda(package) => package.build(context),
            DistTarget::Docker(package) => package.build(context),
        }
    }

    pub fn publish(&self, context: &Context) -> Result<()> {
        match self {
            DistTarget::AwsLambda(package) => package.publish(context),
            DistTarget::Docker(package) => package.publish(context),
        }
    }
}

impl Display for DistTarget<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistTarget::AwsLambda(package) => write!(f, "{}", package),
            DistTarget::Docker(package) => write!(f, "{}", package),
        }
    }
}
