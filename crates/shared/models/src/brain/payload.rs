//! The typed payloads the document store carries in a brain event, so the
//! producer and the consumer agree on one decoded shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId,
    DocumentRevisionId, SessionId, TaskId, TraceId, UserId,
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
