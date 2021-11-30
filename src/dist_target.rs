use std::fmt::Display;

use cargo_metadata::Package;

use crate::Result;

/// A set of build options that can affect the packaging process.
#[derive(Default)]
pub struct BuildOptions {
    pub dry_run: bool,
    pub force: bool,
    pub verbose: bool,
}

pub(crate) enum BuildResult {
    Success,
    Ignored(String),
}

pub(crate) trait DistTarget: Display {
    fn package(&self) -> &Package;
    fn build(&self, options: &BuildOptions) -> Result<BuildResult>;
}
