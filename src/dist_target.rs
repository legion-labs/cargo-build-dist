use std::fmt::Display;

use cargo_metadata::Package;

use crate::Result;

#[derive(Default)]
pub struct BuildOptions {
    pub dry_run: bool,
    pub verbose: bool,
}

pub trait DistTarget: Display {
    fn package(&self) -> &Package;
    fn build(&self, options: &BuildOptions) -> Result<()>;
}
