//! Requests and outcomes of [`super::CollabService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, DocumentRevisionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinMode {
    Edit,
    View,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub session_id: DocumentEditSessionId,
    pub snapshot: Vec<u8>,
    pub version: Vec<u8>,
    pub head_revision: Option<DocumentRevisionId>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Left {
    pub closed: bool,
    pub checkpoint: Option<DocumentRevisionId>,
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
