//! The typed payloads the document store and the gateway carry in a brain
//! event, so the producer and the consumer agree on one decoded shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AiRequestId, ContentHash, ContextId, DocumentCommentId, DocumentFolderId, DocumentId,
    DocumentNodeId, DocumentRevisionId, GatewayConversationId, SessionId, TaskId, TraceId, UserId,
};

use crate::documents::{ActorKind, RevisionOriginKind};

/// A revision that moved its document's head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionLanded {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub actor_kind: ActorKind,
    pub origin_kind: RevisionOriginKind,
    pub author: UserId,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub trace_id: Option<TraceId>,
    pub task_id: Option<TaskId>,
    pub on_behalf_of: Option<UserId>,
    pub folder_id: Option<DocumentFolderId>,
    pub content_sha256: ContentHash,
}

/// Where a resolved comment was anchored: its stable node and, optionally,
/// the `nth` occurrence of a quote inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentAnchorRef {
    pub node_id: DocumentNodeId,
    pub hash8: Option<String>,
    pub find: Option<String>,
    pub nth: Option<i32>,
}

/// A comment that was resolved, with the head it was resolved against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentResolved {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub comment_author: UserId,
    pub resolved_by: UserId,
    pub anchor: CommentAnchorRef,
    pub head_revision_id: Option<DocumentRevisionId>,
    pub answering_revision_id: Option<DocumentRevisionId>,
    pub folder_id: Option<DocumentFolderId>,
}

/// A gateway turn that completed: the last user text and the assistant's
/// answer, each bounded to the profile's `capture.session.max_chars`.
///
/// Scaffolding for the session pathway: the gateway emits it only when
/// `brain.capture.session.enabled` is on and `capture.sources` lists
/// `session`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTurnClosed {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub ai_request_id: AiRequestId,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub context_id: Option<ContextId>,
    pub user_text: String,
    pub assistant_text: String,
    pub model: String,
    pub client: String,
}

impl SessionTurnClosed {
    /// Keeps the first `max_chars` characters of `text`, on a character
    /// boundary, so a bounded payload never splits a multi-byte character.
    #[must_use]
    pub fn bounded(text: &str, max_chars: usize) -> String {
        text.chars().take(max_chars).collect()
    }
}
