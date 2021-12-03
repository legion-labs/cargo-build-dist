use std::fmt::Display;

use cargo_metadata::Package;

use crate::Result;

/// A set of build options that can affect the packaging process.
#[derive(Default)]
pub struct Options {
    pub dry_run: bool,
    pub force: bool,
    pub verbose: bool,
    pub mode: Mode,
}

pub(crate) trait DistTarget: Display {
    fn package(&self) -> &Package;
    fn build(&self, options: &Options) -> Result<()>;
    fn publish(&self, options: &Options) -> Result<()>;
}

/// A build mode that can either be `Debug` or `Release`.
#[derive(Debug, Clone)]
pub enum Mode {
    Debug,
    Release,
}

impl Mode {
    pub fn from_release_flag(release_flag: bool) -> Self {
        if release_flag {
            Self::Release
        } else {
            Self::Debug
        }
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self::Debug
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
        }
    }
}
