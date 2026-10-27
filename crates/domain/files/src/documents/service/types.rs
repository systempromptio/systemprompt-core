//! Requests and outcomes of [`super::DocumentService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{
    DocumentCommentId, DocumentEditSessionId, DocumentFolderId, DocumentId, DocumentOpId,
    DocumentRevisionId, DocumentTemplateId, DriveDeviceId, McpExecutionId, SessionId,
};
use systemprompt_models::documents::{
    AccessLevel, ActorKind, DocumentFormat, EditOp, Projection, RevisionOriginKind,
};
use systemprompt_provider_contracts::documents::TemplateParams;
use systemprompt_traits::ByteRange;

use crate::documents::models::{
    Document, DocumentFolder, DocumentRevision, DocumentSearchHit, ListCursor, ListScope, ListSort,
    ListedDocument, ListingCursor, LockHolderKind, RevisionAttribution, WriteAck,
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
    pub scope: ListScope,
    pub folder: Option<DocumentFolderId>,
    pub query: Option<String>,
    pub sort: ListSort,
    pub cursor: Option<ListingCursor>,
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
    pub documents: Vec<ListedDocument>,
    pub next_cursor: Option<ListingCursor>,
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
    pub comments: CommentPolicy,
    pub op_id: Option<DocumentOpId>,
}

/// How a commit treats comment anchors.
///
/// `guard` refuses a commit that drops a comment's node or quoted text with
/// `AnchorsAffected`; `detach` lets it land and clears those quotes.
/// `answering` records the new revision on that comment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentPolicy {
    pub allow_detach: bool,
    pub answering: Option<DocumentCommentId>,
}

impl CommentPolicy {
    #[must_use]
    pub const fn detach() -> Self {
        Self {
            allow_detach: true,
            answering: None,
        }
    }
}

/// Something that failed after a revision became the head. The commit
/// stands; the cause is logged where it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitWarning {
    SearchIndexStale,
    EventsNotDelivered,
}

/// A revision a commit landed, with the acknowledgement stored for it.
/// `projection` is the new head's id-addressed projection; a replayed
/// acknowledgement (`replayed`) carries none.
#[derive(Debug, Clone, PartialEq)]
pub struct LandedCommit {
    pub revision: DocumentRevision,
    pub ack: WriteAck,
    pub projection: Option<Projection>,
    pub replayed: bool,
    pub warnings: Vec<CommitWarning>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitOutcome {
    Committed(Box<LandedCommit>),
    Merged(Box<LandedCommit>),
    Conflict {
        head: Box<DocumentRevision>,
        base: Option<DocumentRevisionId>,
    },
}

impl CommitOutcome {
    #[must_use]
    pub fn landed(&self) -> Option<&LandedCommit> {
        match self {
            Self::Committed(landed) | Self::Merged(landed) => Some(landed),
            Self::Conflict { .. } => None,
        }
    }
}
