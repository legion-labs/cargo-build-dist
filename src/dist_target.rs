use std::fmt::Display;

use cargo_metadata::Package;

pub trait DistTarget: Display {
    fn package(&self) -> &Package;
}
