//! What the engine tells the shell besides store changes: run status,
//! permission and question requests, failures, and changes to the model and MCP
//! lists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use harness_ids::{CallId, PermissionId, QuestionId, SessionId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

use crate::error::{EngineError, ErrorKind};

/// Where a session's run stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunStatus {
    Idle,
    Busy,
    Retry {
        attempt: u32,
        message: String,
        next_ms: u64,
    },
}

/// A tool call waiting for the user's approval. An "always" answer remembers
/// the `always` patterns for the rest of the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionAsk {
    pub id: PermissionId,
    pub session: SessionId,
    pub call_id: CallId,
    pub tool: String,
    pub permission: String,
    pub patterns: Vec<String>,
    pub always: Vec<String>,
    pub title: String,
    // JSON: the details the tool attached to its permission request, shaped by the permission.
    pub metadata: Value,
}

/// The user's answer to a [`PermissionAsk`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Once,
    Always,
    Reject { feedback: Option<String> },
}

/// One option of a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

/// One question the model asks the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    #[expect(
        clippy::struct_field_names,
        reason = "`question` is the key the model writes in the question tool's input"
    )]
    pub question: String,
    #[serde(default)]
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: bool,
}

/// Questions waiting for the user's answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionAsk {
    pub id: QuestionId,
    pub session: SessionId,
    pub call_id: CallId,
    pub questions: Vec<Question>,
}

/// A run failure, ready to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFailure {
    pub kind: ErrorKind,
    pub message: String,
}

impl From<&EngineError> for SessionFailure {
    fn from(error: &EngineError) -> Self {
        Self {
            kind: error.kind(),
            message: error.describe(),
        }
    }
}

/// Everything the shell hears from the engine. Message and part changes come
/// from the store's own feed.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    Status {
        session: SessionId,
        status: RunStatus,
    },
    PermissionAsked(Box<PermissionAsk>),
    PermissionResolved {
        id: PermissionId,
        session: SessionId,
        reply: Reply,
    },
    QuestionAsked(QuestionAsk),
    QuestionResolved {
        id: QuestionId,
        session: SessionId,
        answered: bool,
    },
    SessionError {
        session: SessionId,
        error: SessionFailure,
    },
    McpChanged {
        directory: PathBuf,
    },
    ModelsChanged,
}

pub(crate) fn publish(events: &broadcast::Sender<EngineEvent>, event: EngineEvent) {
    if events.send(event).is_err() {
        tracing::trace!("engine event dropped with no subscriber");
    }
}
