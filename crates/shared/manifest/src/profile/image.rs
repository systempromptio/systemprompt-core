//! The runtime image `cloud dockerfile` renders for a profile: its base image
//! and the OS packages a deployment needs beyond core's own.
//!
//! Without this block a re-render could only reproduce core's defaults, so a
//! profile whose image carried extra packages (a document converter, a
//! database client) lost them on every regeneration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const DEFAULT_RUNTIME_BASE_IMAGE: &str = "debian:trixie-slim";

/// `cloud.image`: the base image and the extra apt packages of the runtime
/// image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeImageConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
}

impl RuntimeImageConfig {
    #[must_use]
    pub fn base_image(&self) -> &str {
        self.base.as_deref().unwrap_or(DEFAULT_RUNTIME_BASE_IMAGE)
    }
}
