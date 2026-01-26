use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactListOutput {
    pub artifacts: Vec<ArtifactSummary>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactSummary {
    pub id: String,
    pub name: Option<String>,
    pub artifact_type: String,
    pub tool_name: Option<String>,
    pub task_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactDetailOutput {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub artifact_type: String,
    pub tool_name: Option<String>,
    pub source: Option<String>,
    pub task_id: String,
    pub context_id: String,
    pub skill_id: Option<String>,
    pub skill_name: Option<String>,
    pub mcp_execution_id: Option<String>,
    pub fingerprint: Option<String>,
    pub created_at: String,
    pub parts: Vec<ArtifactPartOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendering_hints: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactPartOutput {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}
