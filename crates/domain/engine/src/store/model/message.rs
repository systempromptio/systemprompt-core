//! Messages of either role, their failures, and windows over a session's
//! history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::id::{MessageId, SessionId, now_ms};
use crate::model::{FinishReason, Part, Usage};

/// The class of failure an assistant message ended with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageErrorKind {
    Aborted,
    Auth,
    OutputLength,
    ContextOverflow,
    ContentFilter,
    Governance,
    Api,
    Unknown,
}

/// The failure an assistant message ended with, as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageError {
    pub kind: MessageErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default)]
    pub retryable: bool,
}

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// A prompt. `system` marks one the engine wrote itself (a continuation or
/// compaction request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMessage {
    pub id: MessageId,
    pub session_id: SessionId,
    pub agent: String,
    pub model: String,
    #[serde(default)]
    pub system: bool,
    pub created_at: i64,
}

impl UserMessage {
    pub fn new(session_id: SessionId, agent: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: MessageId::mint(),
            session_id,
            agent: agent.into(),
            model: model.into(),
            system: false,
            created_at: now_ms(),
        }
    }
}

/// A model reply to the user message `parent_id`. `system` marks a compaction
/// summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: MessageId,
    pub session_id: SessionId,
    pub parent_id: MessageId,
    pub agent: String,
    pub model: String,
    #[serde(default)]
    pub system: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<MessageError>,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<FinishReason>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
}

impl AssistantMessage {
    pub fn new(
        session_id: SessionId,
        parent_id: MessageId,
        agent: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: MessageId::mint(),
            session_id,
            parent_id,
            agent: agent.into(),
            model: model.into(),
            system: false,
            error: None,
            usage: Usage::default(),
            finish: None,
            created_at: now_ms(),
            completed_at: None,
        }
    }
}

/// A message of either role; its content lives in its parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}

impl Message {
    pub const fn id(&self) -> &MessageId {
        match self {
            Self::User(message) => &message.id,
            Self::Assistant(message) => &message.id,
        }
    }

    pub const fn session_id(&self) -> &SessionId {
        match self {
            Self::User(message) => &message.session_id,
            Self::Assistant(message) => &message.session_id,
        }
    }

    pub const fn role(&self) -> Role {
        match self {
            Self::User(_) => Role::User,
            Self::Assistant(_) => Role::Assistant,
        }
    }

    pub fn agent(&self) -> &str {
        match self {
            Self::User(message) => &message.agent,
            Self::Assistant(message) => &message.agent,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::User(message) => &message.model,
            Self::Assistant(message) => &message.model,
        }
    }

    pub const fn is_system(&self) -> bool {
        match self {
            Self::User(message) => message.system,
            Self::Assistant(message) => message.system,
        }
    }

    pub const fn created_at(&self) -> i64 {
        match self {
            Self::User(message) => message.created_at,
            Self::Assistant(message) => message.created_at,
        }
    }

    pub const fn as_user(&self) -> Option<&UserMessage> {
        match self {
            Self::User(message) => Some(message),
            Self::Assistant(_) => None,
        }
    }

    pub const fn as_assistant(&self) -> Option<&AssistantMessage> {
        match self {
            Self::User(_) => None,
            Self::Assistant(message) => Some(message),
        }
    }
}

impl From<UserMessage> for Message {
    fn from(message: UserMessage) -> Self {
        Self::User(message)
    }
}

impl From<AssistantMessage> for Message {
    fn from(message: AssistantMessage) -> Self {
        Self::Assistant(message)
    }
}

/// A message with its parts in order.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageWithParts {
    pub message: Message,
    pub parts: Vec<Part>,
}

/// A window over a session's messages: the newest `limit` messages older than
/// `before`, returned oldest first. The default is every message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagePage {
    pub before: Option<MessageId>,
    pub limit: Option<usize>,
}

impl MessagePage {
    pub fn all() -> Self {
        Self::default()
    }

    pub const fn latest(limit: usize) -> Self {
        Self {
            before: None,
            limit: Some(limit),
        }
    }

    pub fn before(mut self, message_id: MessageId) -> Self {
        self.before = Some(message_id);
        self
    }
}
