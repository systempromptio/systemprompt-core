//! Serializable output shapes for the `core artifacts` commands.
//!
//! Defines [`ArtifactListOutput`] and [`ArtifactPartOutput`] (and re-exports
//! the shared [`ArtifactSummary`]) so list/show renderers emit a stable JSON
//! schema.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub use systemprompt_models::a2a::ArtifactSummary;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactListOutput {
    pub artifacts: Vec<ArtifactSummary>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none", rename = "context_id")]
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactPartOutput {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}
