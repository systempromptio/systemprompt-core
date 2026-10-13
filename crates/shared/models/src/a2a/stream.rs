//! A2A streaming and send-message result wire types: task status and
//! artifact update events, the `StreamResponse` oneof, and the
//! `SendMessageResponse` oneof.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, TaskId};

use super::artifact::Artifact;
use super::message::Message;
use super::task::{Task, TaskStatus};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    // JSON: A2A `TaskStatusUpdateEvent.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl TaskStatusUpdateEvent {
    #[must_use]
    pub const fn new(task_id: TaskId, context_id: ContextId, status: TaskStatus) -> Self {
        Self {
            task_id,
            context_id,
            status,
            metadata: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub artifact: Artifact,
    #[serde(default)]
    pub append: bool,
    #[serde(default)]
    pub last_chunk: bool,
    // JSON: A2A `TaskArtifactUpdateEvent.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl TaskArtifactUpdateEvent {
    #[must_use]
    pub const fn new(task_id: TaskId, context_id: ContextId, artifact: Artifact) -> Self {
        Self {
            task_id,
            context_id,
            artifact,
            append: false,
            last_chunk: true,
            metadata: None,
        }
    }

    #[must_use]
    pub const fn chunk(mut self, append: bool, last_chunk: bool) -> Self {
        self.append = append;
        self.last_chunk = last_chunk;
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StreamResponse {
    Task(Task),
    Message(Message),
    StatusUpdate(TaskStatusUpdateEvent),
    ArtifactUpdate(TaskArtifactUpdateEvent),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SendMessageResult {
    Task(Box<Task>),
    Message(Box<Message>),
}
