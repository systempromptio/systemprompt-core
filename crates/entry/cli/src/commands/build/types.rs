//! Build-command argument and summary types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildExtensionRow {
    pub name: String,
    pub build_type: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildOutput {
    pub extensions: Vec<BuildExtensionRow>,
    pub total: usize,
    pub successful: usize,
    pub release_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoreBuildOutput {
    pub target: String,
    pub mode: String,
    pub status: String,
    pub duration_secs: Option<f64>,
}
