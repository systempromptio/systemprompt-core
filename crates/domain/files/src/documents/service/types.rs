//! Requests and outcomes of [`super::DocumentService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{
    DocumentEditSessionId, DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId,
    DriveDeviceId, McpExecutionId, SessionId,
};
use systemprompt_models::documents::{
    AccessLevel, ActorKind, DocumentFormat, EditOp, RevisionOriginKind,
};
use systemprompt_provider_contracts::documents::TemplateParams;
use systemprompt_traits::ByteRange;

use crate::documents::models::{
    Document, DocumentFolder, DocumentRevision, DocumentSearchHit, ListCursor, LockHolderKind,
    RevisionAttribution,
};
use crate::documents::revision::CommitBody;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionOrigin {
    McpExecution(McpExecutionId),
    BridgeDevice(DriveDeviceId),
    WebSession(SessionId),
    EditSession(DocumentEditSessionId),
    Api,
}

impl RevisionOrigin {
    #[must_use]
    pub const fn kind(&self) -> RevisionOriginKind {
        match self {
            Self::McpExecution(_) => RevisionOriginKind::McpExecution,
            Self::BridgeDevice(_) => RevisionOriginKind::BridgeDevice,
            Self::WebSession(_) => RevisionOriginKind::WebSession,
            Self::EditSession(_) => RevisionOriginKind::EditSession,
            Self::Api => RevisionOriginKind::Api,
        }
    }

    #[must_use]
    pub fn id(&self) -> Option<String> {
        match self {
            Self::McpExecution(id) => Some(id.as_str().to_owned()),
            Self::BridgeDevice(id) => Some(id.as_str().to_owned()),
            Self::WebSession(id) => Some(id.as_str().to_owned()),
            Self::EditSession(id) => Some(id.as_str().to_owned()),
            Self::Api => None,
        }
    }

    #[must_use]
    pub const fn device(&self) -> Option<&DriveDeviceId> {
        match self {
            Self::BridgeDevice(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub actor_kind: ActorKind,
    pub origin: RevisionOrigin,
    pub attribution: RevisionAttribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFolderRequest {
    pub parent: Option<DocumentFolderId>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListFolderRequest {
    pub folder: Option<DocumentFolderId>,
    pub cursor: Option<ListCursor>,
    pub limit: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryPage {
    pub limit: i64,
    pub before: Option<ListCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamRequest<'a> {
    pub document: &'a DocumentId,
    pub revision: Option<&'a DocumentRevisionId>,
    pub range: Option<ByteRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireLockRequest {
    pub holder_kind: LockHolderKind,
    pub holder_id: String,
    pub ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub folders: Vec<DocumentFolder>,
    pub documents: Vec<Document>,
    pub next_cursor: Option<ListCursor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateRequest {
    pub template_id: DocumentTemplateId,
    pub folder: Option<DocumentFolderId>,
    pub name: String,
    pub params: TemplateParams,
    pub ops: Vec<EditOp>,
    pub author: Author,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRequest {
    pub folder: Option<DocumentFolderId>,
    pub name: String,
    pub format: Option<DocumentFormat>,
    pub bytes: Vec<u8>,
    pub author: Author,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentView {
    pub document: Document,
    pub head: Option<DocumentRevision>,
    pub my_access: AccessLevel,
    pub folder_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub document: Document,
    pub hit: DocumentSearchHit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitRequest {
    pub document_id: DocumentId,
    pub base_revision: Option<DocumentRevisionId>,
    pub body: CommitBody,
    pub author: Author,
    pub message: Option<String>,
    pub lock_holder_id: Option<String>,
}

/// Something that failed after a revision became the head. The commit
/// stands; the cause is logged where it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitWarning {
    SearchIndexStale,
    EventsNotDelivered,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitOutcome {
    Committed {
        revision: DocumentRevision,
        warnings: Vec<CommitWarning>,
    },
    Merged {
        revision: DocumentRevision,
        merged_from: DocumentRevisionId,
        warnings: Vec<CommitWarning>,
    },
    Conflict {
        head: DocumentRevision,
        base: Option<DocumentRevisionId>,
    },
}
