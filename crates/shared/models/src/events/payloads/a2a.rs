//! A2A event payload shapes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    // JSON: A2A `Message` input as sent by the client.
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
    // JSON: A2A `Part` content as emitted by the agent.
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
    // JSON: A2A JSON-RPC 2.0 envelope (`id` may be a string or a number).
    pub id: Value,
    // JSON: A2A JSON-RPC 2.0 envelope (`id` may be a string or a number).
    pub result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcErrorPayload {
    // JSON: A2A JSON-RPC 2.0 envelope (`id` may be a string or a number).
    pub id: Value,
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // JSON: A2A `DataPart.data` is spec-defined as a free-form object.
    pub data: Option<Value>,
}
