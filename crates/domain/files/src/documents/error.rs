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
    ContentHash, DocumentFolderId, DocumentId, DocumentNodeId, DocumentRevisionId,
    DocumentTemplateId,
};
use systemprompt_models::authz::SubjectSourceError;
use systemprompt_models::documents::{AccessLevel, DocumentAction, DocumentFormat};
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError, TargetError};
use systemprompt_traits::RepositoryError;

use super::blob_store::BlobStoreError;
use super::models::{ListCursorError, LockHolderKind};
use super::templates::TemplateError;

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
        policy_label(.policy.as_deref())
    )]
    Forbidden {
        action: DocumentAction,
        needed: AccessLevel,
        held: Option<AccessLevel>,
        policy: Option<String>,
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
        kinds_label(.op, .accepted_kinds)
    )]
    UnsupportedOp {
        op: &'static str,
        format: DocumentFormat,
        accepted_kinds: &'static [&'static str],
    },
    #[error("no codec is registered for {format} documents")]
    UnsupportedFormat { format: DocumentFormat },
    #[error("cannot tell the document format of \"{name}\"; give the name a known extension")]
    UnknownFormat { name: String },
    #[error("{what} {id} does not exist or has been deleted")]
    NotFound { what: &'static str, id: String },
    #[error("content of {size} bytes exceeds the {limit}-byte document limit")]
    TooLarge { size: u64, limit: u64 },
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
    #[error("document codec failed")]
    Codec(#[source] CodecError),
    #[error("document template {template_id} failed: {source}")]
    Template {
        template_id: DocumentTemplateId,
        #[source]
        source: TemplateError,
    },
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

impl From<CodecError> for DocumentsError {
    fn from(error: CodecError) -> Self {
        match error {
            CodecError::StaleAnchor {
                node,
                expected,
                actual,
                current_text,
            } => Self::StaleAnchor {
                node,
                expected,
                actual,
                current_text,
            },
            CodecError::UnknownNode { node } => Self::UnknownNode { node },
            other => Self::Codec(other),
        }
    }
}

impl From<TargetError> for DocumentsError {
    fn from(error: TargetError) -> Self {
        match error {
            TargetError::FindNone { find, closest } => Self::FindNone { find, closest },
            TargetError::UnknownNode { node } => Self::UnknownNode { node },
            TargetError::MissingHash { .. } => Self::Codec(CodecError::Malformed {
                detail: "expected_hash is required when node names a node id",
            }),
        }
    }
}

impl From<SubjectSourceError> for DocumentsError {
    fn from(error: SubjectSourceError) -> Self {
        Self::Authz(AuthzFailure::Subjects(error))
    }
}

impl From<AuthzError> for DocumentsError {
    fn from(error: AuthzError) -> Self {
        Self::Authz(AuthzFailure::Catalog(error))
    }
}

impl DocumentsError {
    pub(crate) fn not_found(what: &'static str, id: impl std::fmt::Display) -> Self {
        Self::NotFound {
            what,
            id: id.to_string(),
        }
    }
}

fn base_label(base: Option<&DocumentRevisionId>) -> String {
    base.map_or_else(
        || "an unknown base revision".to_owned(),
        |base| format!("base revision {base}"),
    )
}

fn kinds_label(op: &str, kinds: &[&str]) -> String {
    match (op, kinds) {
        ("insert_after", []) => "; this format takes no insert_after".to_owned(),
        ("insert_after", kinds) => format!("; insert_after accepts kind {}", kinds.join(", ")),
        _ => String::new(),
    }
}

fn closest_label(closest: Option<&ClosestBlock>) -> String {
    closest.map_or_else(String::new, |block| {
        format!(
            "; the closest block is {} (hash {}): \"{}\"",
            block.node, block.hash, block.text
        )
    })
}

fn held_label(held: Option<&AccessLevel>) -> String {
    held.map_or_else(|| "no access".to_owned(), |level| format!("{level} access"))
}

fn policy_label(policy: Option<&str>) -> String {
    policy.map_or_else(
        || ". Ask the owner to share it with you at that level".to_owned(),
        |policy| format!(" but policy {policy} denied the request"),
    )
}
