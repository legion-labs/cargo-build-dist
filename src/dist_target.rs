use cargo_metadata::Package;

pub trait DistTarget {
    fn package(&self) -> &Package;
}
