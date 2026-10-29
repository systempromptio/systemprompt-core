//! Requests and outcomes of [`super::CollabService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{
    DocumentEditSessionId, DocumentId, DocumentProposalId, DocumentRevisionId,
};
use systemprompt_models::documents::AccessLevel;

/// How a client joins: `edit` needs edit access, `comment` comment access
/// (its updates may only add or remove comment marks), `view` read access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinMode {
    Edit,
    Comment,
    View,
}

/// Protocol v3 join result.
///
/// The session state plus what a client needs to bind an
/// editor to it — the engine and wire version it must speak, the model and
/// its schema version, and the caller's access level. `peer` is the engine
/// peer id the server assigned to this join; the client's editor must use it,
/// and an update carrying operations of a peer the caller was not assigned is
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub session_id: DocumentEditSessionId,
    pub snapshot: Vec<u8>,
    pub version: Vec<u8>,
    pub seq: i64,
    pub head_revision: Option<DocumentRevisionId>,
    pub engine: &'static str,
    pub wire_version: &'static str,
    pub model: &'static str,
    pub model_version: u32,
    pub level: AccessLevel,
    pub peer: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    pub document_id: DocumentId,
    pub session_id: DocumentEditSessionId,
    pub version: Option<Vec<u8>>,
    pub update: Vec<u8>,
}

#[derive(Debug)]
pub struct CursorRequest {
    pub document: DocumentId,
    pub session: DocumentEditSessionId,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    pub cursor: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub seq: i64,
    pub version: Vec<u8>,
    pub missing: Option<Vec<u8>>,
}

/// The outcome of committing a session's live body.
///
/// A conflicting body is kept as a proposal on the session's checkpoint base
/// when the session is being left or closed, and is otherwise retried at the
/// next checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checkpoint {
    Unchanged,
    Committed(DocumentRevisionId),
    Conflict {
        head: DocumentRevisionId,
        proposal: Option<DocumentProposalId>,
    },
}

impl Checkpoint {
    #[must_use]
    pub const fn revision(&self) -> Option<&DocumentRevisionId> {
        match self {
            Self::Committed(revision) => Some(revision),
            Self::Unchanged | Self::Conflict { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Left {
    pub closed: bool,
    pub checkpoint: Checkpoint,
}

/// When a replica snapshots and checkpoints the sessions it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollabTuning {
    pub snapshot_after: Duration,
    pub snapshot_after_updates: u32,
    pub checkpoint_after: Duration,
    pub tick: Duration,
    pub evict_after: Duration,
}

impl Default for CollabTuning {
    fn default() -> Self {
        Self {
            snapshot_after: Duration::from_secs(30),
            snapshot_after_updates: 200,
            checkpoint_after: Duration::from_secs(120),
            tick: Duration::from_secs(5),
            evict_after: Duration::from_secs(600),
        }
    }
}
