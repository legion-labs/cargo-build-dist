//! Metadata structures for the various targets.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

use crate::docker::DockerMetadata;

/// The root metadata structure.
#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub deps_hash: Option<String>,
    #[serde(flatten)]
    pub targets: HashMap<String, Target>,
}

#[derive(Debug, Clone)]
pub enum Target {
    Docker(crate::docker::DockerMetadata),
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        enum TargetType {
            #[serde(rename = "docker")]
            Docker,
        }

        #[derive(Deserialize)]
        struct TargetHelper {
            #[serde(rename = "type")]
            target_type: TargetType,
            #[serde(flatten)]
            data: serde_value::Value,
        }

        let helper = TargetHelper::deserialize(deserializer)?;
        match helper.target_type {
            TargetType::Docker => DockerMetadata::deserialize(helper.data)
                .map(Target::Docker)
                .map_err(serde::de::Error::custom),
        }
    }
}
