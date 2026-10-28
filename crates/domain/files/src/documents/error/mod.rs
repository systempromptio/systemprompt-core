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

use systemprompt_events::RelayError;
use systemprompt_governance::authz::AuthzError;
use systemprompt_identifiers::{
    ContentHash, DocumentAccessRequestId, DocumentCommentId, DocumentFolderId, DocumentId,
    DocumentNodeId, DocumentOpId, DocumentProposalId, DocumentRevisionId, DocumentTemplateId,
    UserId,
};
use systemprompt_models::authz::SubjectSourceError;
use systemprompt_models::documents::{
    AccessLevel, AccessRequestStatus, DocumentAction, DocumentFormat, ProposalStatus,
};
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};
use systemprompt_traits::RepositoryError;

use self::labels::{
    base_label, closest_label, comment_list, conflict_label, held_label, kinds_label, policy_label,
};
use super::access::SubjectDirectoryError;
use super::blob_store::BlobStoreError;
use super::models::{ListCursorError, LockHolderKind};
use super::templates::TemplateError;

mod conversions;
mod labels;

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
    #[error(
        "node {node} changed since it was read (expected hash {expected}, now {actual}); its \
         current text is \"{current_text}\". The head edit wins: re-aim your change at this \
         text with hash {actual}, and never resend your old text over it"
    )]
    StaleAnchor {
        node: DocumentNodeId,
        expected: String,
        actual: String,
        current_text: String,
    },
    #[error(
        "node {node} does not exist at the head revision (deleted, or not an id of this \
         document); read the outline again for current node ids"
    )]
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
    #[error(
        "document {document_id} is in the trash; restore it with documents_untrash \
         before reading, editing or sharing it"
    )]
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
        "document {document_id} is locked by {holder_kind} holder {holder_id}; wait for the lock \
         to be released or to expire"
    )]
    LockHeld {
        document_id: DocumentId,
        holder_kind: LockHolderKind,
        holder_id: String,
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
    #[error(
        "op id {op_id} was already used for a different request; send a new op id for a new \
         write"
    )]
    OpIdMismatch { op_id: DocumentOpId },
    #[error(
        "{op} on node {node} needs expected_hash, the node's hash8 from your last read; a quote \
         target resolves its own"
    )]
    GuardMissing {
        op: &'static str,
        node: DocumentNodeId,
    },
    #[error("proposal {proposal_id} was proposed by the reviewer; another editor must review it")]
    SelfReview { proposal_id: DocumentProposalId },
    #[error(
        "uploaded bytes hash to {actual} but the commit declared {declared}; the upload was \
         corrupted, send it again"
    )]
    IntegrityMismatch {
        declared: ContentHash,
        actual: ContentHash,
    },
    #[error(
        "\"{name}\" is not a valid name: use 1-255 characters without '/', '\\', control \
         characters, \".\" or \"..\""
    )]
    InvalidName { name: String },
    #[error("subject kind \"{kind}\" is not user, role or a registered subject dimension")]
    UnknownSubjectKind { kind: String },
    #[error(
        "no active {kind} \"{value}\" exists; name an existing, active subject (a user id for \
         a user)"
    )]
    UnknownSubject { kind: String, value: String },
    #[error(
        "own is not a grant level: it belongs to the owner alone. Grant share to let someone \
         manage access, or transfer ownership"
    )]
    OwnNotGrantable,
    #[error(
        "forbidden: the subject already holds {existing} here, above your {held}; only a holder \
         of {existing} or the owner can change that grant"
    )]
    ShareBelowExisting {
        existing: AccessLevel,
        held: AccessLevel,
        owner: Option<UserId>,
    },
    #[error("folder {folder_id} is not empty; delete its contents first or delete recursively")]
    FolderNotEmpty { folder_id: DocumentFolderId },
    #[error("folder {folder_id} cannot move into itself or one of its own subfolders")]
    InvalidMove { folder_id: DocumentFolderId },
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
    #[error(
        "you already have an open access request on document {document_id}; wait for the \
         owner to answer it"
    )]
    AccessRequestOpen { document_id: DocumentId },
    #[error(
        "you already hold {held} access to document {document_id}; ask someone who holds share \
         to raise it"
    )]
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
}

#[derive(Debug, thiserror::Error)]
pub enum AuthzFailure {
    #[error("subject attributes could not be resolved")]
    Subjects(#[from] SubjectSourceError),
    #[error("access control catalog could not be updated")]
    Catalog(#[from] AuthzError),
}

#[derive(Debug, thiserror::Error)]
pub enum EventFailure {
    #[error("event payload could not be serialised")]
    Serialize(#[from] serde_json::Error),
    #[error("event could not be relayed to other replicas")]
    Relay(#[from] RelayError),
}

impl DocumentsError {
    pub(crate) fn not_found(what: &'static str, id: impl std::fmt::Display) -> Self {
        Self::NotFound {
            what,
            id: id.to_string(),
        }
    }
}
