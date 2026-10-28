//! Payloads of the comment events beyond creation and resolution: an edit
//! of a comment's body or mentions, a deletion, and the notice a mentioned
//! user receives.
//!
//! Like every document event they are also published on the document's live
//! topic, so an open workspace updates its comment sidebar in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, UserId};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentMentionedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub mentioned_user_id: UserId,
    pub author_id: UserId,
}

impl DocumentCommentMentionedEvent {
    pub const NAME: &'static str = "document.comment.mentioned";
}
