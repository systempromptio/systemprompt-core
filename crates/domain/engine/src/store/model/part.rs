//! The ordered pieces a message is made of: text, reasoning, files, tool calls
//! and step marks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::id::{CallId, MessageId, PartId, SessionId, now_ms};
use crate::store::model::{FinishReason, JsonMap, Usage};

/// A file a tool produced alongside its output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub url: String,
}

/// The lifecycle of one tool call. Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolState {
    Pending {
        // JSON: tool arguments as the model sent them; each tool checks them against its schema.
        input: Value,
    },
    Running {
        // JSON: tool arguments as the model sent them.
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        metadata: JsonMap,
        started: i64,
    },
    Completed {
        // JSON: tool arguments as the model sent them.
        input: Value,
        output: String,
        title: String,
        #[serde(default)]
        metadata: JsonMap,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Attachment>,
        started: i64,
        ended: i64,
    },
    Error {
        // JSON: tool arguments as the model sent them.
        input: Value,
        error: String,
        // JSON: what the tool or the engine recorded about the failure, such as who refused a call
        // that never ran.
        #[serde(default)]
        metadata: JsonMap,
        started: i64,
        ended: i64,
    },
}

impl ToolState {
    // JSON: tool arguments as the model sent them.
    pub const fn input(&self) -> &Value {
        match self {
            Self::Pending { input }
            | Self::Running { input, .. }
            | Self::Completed { input, .. }
            | Self::Error { input, .. } => input,
        }
    }

    pub const fn is_finished(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Error { .. })
    }
}

/// The content of a part. The serde tag is also the `kind` column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartBody {
    Text {
        text: String,
        #[serde(default)]
        synthetic: bool,
    },
    Reasoning {
        text: String,
        // JSON: provider replay data (signatures, encrypted reasoning), sent back verbatim.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_data: Option<Value>,
    },
    File {
        mime: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        url: String,
    },
    Tool {
        call_id: CallId,
        tool: String,
        state: Box<ToolState>,
    },
    StepStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<String>,
    },
    StepFinish {
        reason: FinishReason,
        usage: Usage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<String>,
    },
    Patch {
        hash: String,
        files: Vec<String>,
    },
    Compaction {
        auto: bool,
    },
    Subtask {
        agent: String,
        prompt: String,
        description: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        child_session: Option<SessionId>,
    },
}

impl PartBody {
    pub const TEXT: &'static str = "text";
    pub const REASONING: &'static str = "reasoning";

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => Self::TEXT,
            Self::Reasoning { .. } => Self::REASONING,
            Self::File { .. } => "file",
            Self::Tool { .. } => "tool",
            Self::StepStart { .. } => "step_start",
            Self::StepFinish { .. } => "step_finish",
            Self::Patch { .. } => "patch",
            Self::Compaction { .. } => "compaction",
            Self::Subtask { .. } => "subtask",
        }
    }

    pub const fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } | Self::Reasoning { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }
}

/// One ordered piece of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub session_id: SessionId,
    pub message_id: MessageId,
    pub created_at: i64,
    pub updated_at: i64,
    pub body: PartBody,
}

impl Part {
    pub fn new(session_id: SessionId, message_id: MessageId, body: PartBody) -> Self {
        let now = now_ms();
        Self {
            id: PartId::mint(),
            session_id,
            message_id,
            created_at: now,
            updated_at: now,
            body,
        }
    }

    pub const fn kind(&self) -> &'static str {
        self.body.kind()
    }
}
