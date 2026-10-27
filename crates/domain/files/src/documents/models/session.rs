//! Edit session and text index rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    ContentHash, DocumentEditSessionId, DocumentId, DocumentRevisionId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentEditSession {
    pub id: DocumentEditSessionId,
    pub document_id: DocumentId,
    pub base_revision_id: Option<DocumentRevisionId>,
    pub snapshot_sha256: Option<ContentHash>,
    pub checkpoint_revision_id: Option<DocumentRevisionId>,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditSessionOpen {
    Opened(DocumentEditSession),
    AlreadyOpen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTextIndexRow {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub updated_at: DateTime<Utc>,
}
