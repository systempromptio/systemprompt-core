//! Revision and blob rows with the revision insert request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentId, DocumentRevisionId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::documents::{ActorKind, EditOp};
use systemprompt_traits::StoredFileId;

use super::origin::RevisionOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBlob {
    pub content_sha256: ContentHash,
    pub size_bytes: i64,
    pub storage_id: StoredFileId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentRevision {
    pub id: DocumentRevisionId,
    pub document_id: DocumentId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub content_sha256: ContentHash,
    pub author_user_id: UserId,
    pub actor_kind: ActorKind,
    pub origin: RevisionOrigin,
    pub message: Option<String>,
    pub edit_ops: Option<Vec<EditOp>>,
    pub attribution: RevisionAttribution,
    pub created_at: DateTime<Utc>,
    pub landed_at: Option<DateTime<Utc>>,
}

/// The request a revision was written in: the session, conversation context,
/// trace and task, and the user an agent acted for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RevisionAttribution {
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub trace_id: Option<TraceId>,
    pub task_id: Option<TaskId>,
    pub on_behalf_of: Option<UserId>,
}

#[derive(Debug, Clone)]
pub struct NewRevision {
    pub id: DocumentRevisionId,
    pub document_id: DocumentId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub content_sha256: ContentHash,
    pub author_user_id: UserId,
    pub actor_kind: ActorKind,
    pub origin: RevisionOrigin,
    pub message: Option<String>,
    pub edit_ops: Option<Vec<EditOp>>,
    pub attribution: RevisionAttribution,
}
