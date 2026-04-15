use crate::models::a2a::{Artifact, TaskStatus};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    pub kind: String,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    #[serde(rename = "final")]
    pub is_final: bool,
}

impl TaskStatusUpdateEvent {
    pub fn new(
        task_id: impl Into<TaskId>,
        context_id: impl Into<ContextId>,
        status: TaskStatus,
        is_final: bool,
    ) -> Self {
        Self {
            kind: "status-update".to_string(),
            task_id: task_id.into(),
            context_id: context_id.into(),
            status,
            is_final,
        }
    }

    pub fn to_jsonrpc_response(&self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "result": self
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    pub kind: String,
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub artifact: Artifact,
    #[serde(rename = "final")]
    pub is_final: bool,
}

impl TaskArtifactUpdateEvent {
    pub fn new(
        task_id: impl Into<TaskId>,
        context_id: impl Into<ContextId>,
        artifact: Artifact,
        is_final: bool,
    ) -> Self {
        Self {
            kind: "artifact-update".to_string(),
            task_id: task_id.into(),
            context_id: context_id.into(),
            artifact,
            is_final,
        }
    }

    pub fn to_jsonrpc_response(&self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "result": self
        })
    }
}
