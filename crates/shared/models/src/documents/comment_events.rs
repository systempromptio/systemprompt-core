//! Payloads of the comment events beyond creation and resolution: an edit
//! of a comment's body or mentions, a deletion, and the notice a mentioned
//! user receives.
//!
//! Like every document event they are also published on the document's live
//! topic, so an open workspace updates its comment sidebar in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, DocumentCommentId, DocumentId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentUpdatedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub edited_by: UserId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<UserId>,
}

impl DocumentCommentUpdatedEvent {
    pub const NAME: &'static str = "document.comment.updated";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentDeletedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub deleted_by: UserId,
}

impl DocumentCommentDeletedEvent {
    pub const NAME: &'static str = "document.comment.deleted";
}

/// The notice one mentioned user receives.
///
/// It is the input contract of the notifications project
/// (`v` = [`MENTION_EVENT_VERSION`]). `author_id` is
/// the acting user; `via_agent` names the agent that wrote the comment for
/// them. `mentions` is the comment's whole mention list and `granted` the
/// users this comment gave `comment` access. `excerpt` is the start of the
/// body, mention tokens (`@[Name](user:<id>)`) included; the full thread is
/// read by `comment_id`. Fields after `author_id` default when absent, so a
/// frame from an older server still decodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentMentionedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub mentioned_user_id: UserId,
    pub author_id: UserId,
    #[serde(default)]
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_agent: Option<AgentName>,
    #[serde(default)]
    pub mentions: Vec<UserId>,
    #[serde(default)]
    pub granted: Vec<UserId>,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_quote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

pub const MENTION_EVENT_VERSION: u32 = 1;

pub const MENTION_EXCERPT_CHARS: usize = 280;

impl DocumentCommentMentionedEvent {
    pub const NAME: &'static str = "document.comment.mentioned";
}
