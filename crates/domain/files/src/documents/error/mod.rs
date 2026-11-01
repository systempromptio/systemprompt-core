//! [`DocumentsError`]: the one error surface of the document services.
//!
//! The Display strings are read by agents as well as people, so each names
//! what went wrong, the ids involved and what to do next: a stale anchor
//! carries the node's current text so the caller can retry without a full
//! re-read, a conflict names the head revision to rebase on, and a forbidden
//! answer names the level held and the level needed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{BrainSinkError, NotificationSinkError};
use systemprompt_identifiers::{
    ContentHash, DocumentAccessRequestId, DocumentCommentId, DocumentFolderId, DocumentId,
    DocumentNodeId, DocumentOpId, DocumentProposalId, DocumentRevisionId, DocumentTemplateId,
    UserId,
};
use systemprompt_models::documents::{
    AccessLevel, AccessRequestStatus, DocumentAction, DocumentFormat, ProposalStatus,
};
use systemprompt_models::notifications::NotificationKindError;
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};
use systemprompt_traits::RepositoryError;

use self::labels::{
    base_label, closest_label, comment_list, conflict_label, held_label, kinds_label, policy_label,
};
use self::messages::{
    access_request_not_needed, access_request_open, guard_missing, integrity_mismatch,
    invalid_name, live_unsupported, name_taken, op_id_mismatch, ownership_changed,
    share_below_existing, stale_anchor, subtree_changed, trashed, unknown_node, unknown_subject,
};
use super::access::SubjectDirectoryError;
use super::agents::AgentRefusal;
use super::blob_store::BlobStoreError;
use super::models::{ListCursorError, LockHolder};
use super::templates::TemplateError;

mod causes;
mod conversions;
mod labels;
mod messages;

pub use causes::{AuthzFailure, EventFailure};

#[derive(Debug, thiserror::Error)]
pub enum DocumentsError {
    #[error(
        "document {document_id} changed since {}: the head is now revision {head}. Read the \
         document at revision {head} and apply your change again with base_revision {head}",
        base_label(.base.as_ref())
    )]
    Conflict {
        document_id: DocumentId,
        head: DocumentRevisionId,
        base: Option<DocumentRevisionId>,
    },
    #[error("{}", stale_anchor(.node, .expected, .actual, .current_text))]
    StaleAnchor {
        node: DocumentNodeId,
        expected: String,
        actual: String,
        current_text: String,
    },
    #[error("{}", unknown_node(.node))]
    UnknownNode { node: DocumentNodeId },
    #[error(
        "no block contains \"{find}\"{}. Quote text exactly as it appears in a read, or target \
         the node by id",
        closest_label(.closest.as_ref())
    )]
    FindNone {
        find: String,
        closest: Option<ClosestBlock>,
    },
    #[error(
        "forbidden: {action} needs {needed} access and you hold {}{}",
        held_label(.held.as_ref()),
        policy_label(.policy.as_deref(), .owner.as_ref())
    )]
    Forbidden {
        action: DocumentAction,
        needed: AccessLevel,
        held: Option<AccessLevel>,
        policy: Option<String>,
        owner: Option<UserId>,
    },
    #[error("{}", trashed(.document_id))]
    Trashed { document_id: DocumentId },
    #[error(
        "this edit removes text that comments {} are anchored to; pass allow_detach: true to \
         detach them (they keep their node) or re-aim the edit",
        comment_list(.comments)
    )]
    AnchorsAffected { comments: Vec<DocumentCommentId> },
    #[error("invalid comment: {detail}")]
    InvalidComment { detail: &'static str },
    #[error("invalid awareness state: {detail}")]
    InvalidAwareness { detail: &'static str },
    #[error("{detail}")]
    InvalidParams { detail: &'static str },
    #[error("comment {comment_id} does not belong to document {document_id}")]
    CommentNotFound {
        document_id: DocumentId,
        comment_id: DocumentCommentId,
    },
    #[error(
        "document {document_id} is locked by {} holder {}; wait for the lock to be released or to \
         expire",
        .holder.kind(),
        .holder.id_str()
    )]
    LockHeld {
        document_id: DocumentId,
        holder: LockHolder,
    },
    #[error(
        "operation {op} is not supported for {format} documents{}",
        kinds_label(.op, .format, .accepted_kinds)
    )]
    UnsupportedOp {
        op: &'static str,
        format: DocumentFormat,
        accepted_kinds: &'static [&'static str],
    },
    #[error("no codec is registered for {format} documents")]
    UnsupportedFormat { format: DocumentFormat },
    #[error("{}", live_unsupported(.format))]
    LiveUnsupported { format: DocumentFormat },
    #[error("the content was refused ({code}): {detail}")]
    ContentRefused { code: &'static str, detail: String },
    #[error("cannot tell the document format of \"{name}\"; give the name a known extension")]
    UnknownFormat { name: String },
    #[error("{what} {id} does not exist or has been deleted")]
    NotFound { what: &'static str, id: String },
    #[error("{what} of {size} exceeds the limit of {limit}")]
    TooLarge {
        what: &'static str,
        size: u64,
        limit: u64,
    },
    #[error("{}", op_id_mismatch(.op_id))]
    OpIdMismatch { op_id: DocumentOpId },
    #[error("{}", guard_missing(.op, .node))]
    GuardMissing {
        op: &'static str,
        node: DocumentNodeId,
    },
    #[error("proposal {proposal_id} was proposed by the reviewer; another editor must review it")]
    SelfReview { proposal_id: DocumentProposalId },
    #[error("{}", integrity_mismatch(.declared, .actual))]
    IntegrityMismatch {
        declared: ContentHash,
        actual: ContentHash,
    },
    #[error("{}", invalid_name(.name))]
    InvalidName { name: String },
    #[error("{}", name_taken(.name))]
    NameTaken { name: String },
    #[error("subject kind \"{kind}\" is not user, role or a registered subject dimension")]
    UnknownSubjectKind { kind: String },
    #[error("{}", unknown_subject(.kind, .value))]
    UnknownSubject { kind: String, value: String },
    #[error(
        "own is not a grant level: it belongs to the owner alone. Grant share to let someone \
         manage access, or transfer ownership"
    )]
    OwnNotGrantable,
    #[error("{}", share_below_existing(.existing, .held))]
    ShareBelowExisting {
        existing: AccessLevel,
        held: AccessLevel,
        owner: Option<UserId>,
    },
    #[error("folder {folder_id} is not empty; delete its contents first or delete recursively")]
    FolderNotEmpty { folder_id: DocumentFolderId },
    #[error("{}", subtree_changed(.folder_id))]
    SubtreeChanged { folder_id: DocumentFolderId },
    #[error("{}", ownership_changed(.document_id))]
    OwnershipChanged { document_id: DocumentId },
    #[error("folder {folder_id} cannot move into itself or one of its own subfolders")]
    InvalidMove { folder_id: DocumentFolderId },
    #[error("drive cursor \"{cursor}\" is not a feed position; restart from the manifest")]
    CursorReset {
        cursor: String,
        #[source]
        source: ListCursorError,
    },
    #[error("cursor \"{cursor}\" is not valid; restart from the manifest cursor")]
    InvalidCursor {
        cursor: String,
        #[source]
        source: ListCursorError,
    },
    #[error(
        "proposal {proposal_id} no longer applies at head revision {head}{}. Reject it, or \
         propose the change again on revision {head}",
        conflict_label(.node.as_ref(), .current_text.as_deref())
    )]
    ProposalConflict {
        proposal_id: DocumentProposalId,
        head: DocumentRevisionId,
        node: Option<DocumentNodeId>,
        current_text: Option<String>,
    },
    #[error("{}", access_request_open(.document_id))]
    AccessRequestOpen { document_id: DocumentId },
    #[error("{}", access_request_not_needed(.document_id, .held))]
    AccessRequestNotNeeded {
        document_id: DocumentId,
        held: AccessLevel,
    },
    #[error(
        "access request {request_id} is already {status}; only an open request can be answered"
    )]
    AccessRequestClosed {
        request_id: DocumentAccessRequestId,
        status: AccessRequestStatus,
    },
    #[error("proposal {proposal_id} is already {status}; only an open proposal can be reviewed")]
    ProposalClosed {
        proposal_id: DocumentProposalId,
        status: ProposalStatus,
    },
    #[error(transparent)]
    Agent(#[from] AgentRefusal),
    #[error("document codec failed")]
    Codec(#[source] CodecError),
    #[error("document template {template_id} failed: {source}")]
    Template {
        template_id: DocumentTemplateId,
        #[source]
        source: TemplateError,
    },
    #[error("the document's live edit session could not be closed")]
    LiveSession(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("document blob storage failed")]
    Blob(#[from] BlobStoreError),
    #[error("document store query failed")]
    Repository(#[from] RepositoryError),
    #[error("document authorization could not be evaluated")]
    Authz(#[source] AuthzFailure),
    #[error("document event {event} was not delivered")]
    Events {
        event: &'static str,
        #[source]
        source: EventFailure,
    },
    #[error("document worker task failed")]
    Worker(#[from] tokio::task::JoinError),
    #[error("grant subjects could not be resolved to users")]
    Directory(#[from] SubjectDirectoryError),
    #[error("the notification could not be recorded; nothing was stored")]
    Notifications(#[from] NotificationSinkError),
    #[error("notification kind is invalid")]
    NotificationKind(#[from] NotificationKindError),
    #[error("the brain fact could not be recorded; nothing was stored")]
    Brain(#[from] BrainSinkError),
}

impl DocumentsError {
    pub(crate) fn not_found(what: &'static str, id: impl std::fmt::Display) -> Self {
        Self::NotFound {
            what,
            id: id.to_string(),
        }
    }
}
