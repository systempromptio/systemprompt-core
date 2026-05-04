//! Chat message and role primitives shared across LLM providers.

use serde::{Deserialize, Serialize};

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Speaker role for this message.
    pub role: ChatRole,
    /// Message text content.
    pub content: String,
}

impl ChatMessage {
    /// Build a [`ChatRole::User`] message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }

    /// Build a [`ChatRole::Assistant`] message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }

    /// Build a [`ChatRole::System`] message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
}

/// Speaker role attached to a [`ChatMessage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// Out-of-band system instruction.
    System,
    /// End-user message.
    User,
    /// Model-generated reply.
    Assistant,
    /// Tool call result fed back into the conversation.
    Tool,
}
