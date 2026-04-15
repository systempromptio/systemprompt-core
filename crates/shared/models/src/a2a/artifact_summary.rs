use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ArtifactId, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactSummary {
    pub artifact_id: ArtifactId,
    pub name: Option<String>,
    pub artifact_type: String,
    pub tool_name: Option<String>,
    pub task_id: TaskId,
    pub created_at: DateTime<Utc>,
}
