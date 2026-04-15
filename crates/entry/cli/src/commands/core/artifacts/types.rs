use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub use systemprompt_models::a2a::ArtifactSummary;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactListOutput {
    pub artifacts: Vec<ArtifactSummary>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactPartOutput {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}
