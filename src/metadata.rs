//! Metadata structures for the various targets.

use serde::Deserialize;

/// The root metadata structure.
#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub deps_hash: Option<String>,
    pub docker: Option<crate::docker::DockerMetadata>,
}
