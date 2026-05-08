use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};

use crate::a2a::{Artifact, TaskState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSubmittedPayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdatePayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactCreatedPayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactUpdatedPayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub artifact_id: ArtifactId,
    pub append: bool,
    pub last_chunk: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessagePayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub message_id: MessageId,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputRequiredPayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequiredPayload {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcResponsePayload {
    pub id: Value,
    pub result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcErrorPayload {
    pub id: Value,
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
