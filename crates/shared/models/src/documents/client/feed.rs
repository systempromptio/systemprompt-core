//! The Drive sync feed: the folder manifest, the change feed, commits, trash,
//! moves and the sync token the device token is traded for.
//!
//! A commit or a trash names the revision it was based on in `If-Match`; a head
//! that moved on answers `409` with the head, which decodes as the conflict
//! outcome of the same body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, DriveDeviceId};

/// One document of the feed manifest, at its head revision.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeedEntry {
    pub document_id: DocumentId,
    pub path: String,
    pub head_revision_id: DocumentRevisionId,
    pub content_sha256: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Every document the feed mirrors, and the cursor the change feed continues
/// from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeedManifest {
    pub cursor: String,
    #[serde(default)]
    pub entries: Vec<FeedEntry>,
}

/// What happened to a document in one change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeKind {
    Created,
    Updated,
    Renamed {
        #[serde(default)]
        from: Option<String>,
    },
    Deleted,
}

/// One entry of the change feed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeedChange {
    pub cursor: String,
    pub document_id: DocumentId,
    pub path: String,
    #[serde(default)]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(default)]
    pub content_sha256: Option<String>,
    pub kind: ChangeKind,
}

/// A page of the change feed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeedChanges {
    #[serde(default)]
    pub changes: Vec<FeedChange>,
    pub next_cursor: String,
}

/// The `meta` part of a multipart commit; the blob travels beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<DocumentRevisionId>,
    pub content_sha256: String,
    pub size_bytes: u64,
    pub device_id: DriveDeviceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// How a commit ended; a `409` carries the `conflict` form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommitOutcome {
    Committed {
        revision_id: DocumentRevisionId,
    },
    Merged {
        revision_id: DocumentRevisionId,
        #[serde(default)]
        content_sha256: Option<String>,
    },
    Conflict {
        head_revision_id: DocumentRevisionId,
        #[serde(default)]
        head_content_sha256: Option<String>,
        #[serde(default)]
        base_revision_id: Option<DocumentRevisionId>,
    },
    Proposed {
        proposal_id: DocumentProposalId,
        revision_id: DocumentRevisionId,
    },
}

/// How a trash ended; a `409` carries the `conflict` form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TrashOutcome {
    Trashed,
    Conflict {
        head_revision_id: DocumentRevisionId,
        #[serde(default)]
        head_content_sha256: Option<String>,
    },
}

/// Body of a trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TrashRequest<'a> {
    pub document_id: &'a DocumentId,
}

/// Body of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MoveRequest<'a> {
    pub document_id: &'a DocumentId,
    pub path: &'a str,
}
