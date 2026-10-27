//! Revision and blob rows with the revision insert request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId, UserId};
use systemprompt_models::documents::{ActorKind, EditOp, RevisionOriginKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBlob {
    pub content_sha256: ContentHash,
    pub size_bytes: i64,
    pub storage_id: String,
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
    pub origin_kind: RevisionOriginKind,
    pub origin_id: Option<String>,
    pub message: Option<String>,
    pub edit_ops: Option<Vec<EditOp>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewRevision {
    pub document_id: DocumentId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub content_sha256: ContentHash,
    pub author_user_id: UserId,
    pub actor_kind: ActorKind,
    pub origin_kind: RevisionOriginKind,
    pub origin_id: Option<String>,
    pub message: Option<String>,
    pub edit_ops: Option<Vec<EditOp>>,
}
