//! What the Drive folder last agreed with the instance on:
//! `<Drive>/.drive-state.json`.
//!
//! One entry per synced file, keyed by its `/`-separated path: the document,
//! the revision, and the local size, modification time and SHA-256 it had then.
//! The index also keeps the feed cursor, the conflicts kept for the user, edits
//! sent for review, and local deletes waiting to be carried over. It holds no
//! secret. It is written whole and atomically after every step it describes, so
//! an interrupted cycle resumes where it stopped. A file that does not
//! decode is never replaced silently: it is moved aside to
//! `.drive-state.json.corrupt-<time>` and the load fails.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sp_protocol::ids::{DocumentId, DocumentProposalId, DocumentRevisionId};
use sp_secure::{Access, write_atomic};

use super::error::SyncError;

/// The synced form of one file. `document_id` is absent for a file this device
/// created until the feed names the document the instance made for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    pub revision_id: DocumentRevisionId,
    pub sha256: String,
    pub mtime_unix: i64,
    pub size: u64,
}

/// A conflict kept for the user: a local edit kept beside the head as
/// `sibling`, or a document that could not be placed (`unplaced`), whose
/// `sibling` is the file holding the name or the path itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictNotice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unplaced: bool,
    pub path: String,
    pub sibling: String,
    pub head_revision_id: DocumentRevisionId,
    pub detected_at: DateTime<Utc>,
}

/// A local edit the instance took as a proposal: the file stays as edited until
/// a reviewer decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentForReview {
    pub path: String,
    pub proposal_id: DocumentProposalId,
    pub sent_at: DateTime<Utc>,
}

/// A synced file found missing, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDelete {
    pub noticed_at: DateTime<Utc>,
    pub cycles: u32,
}

/// Why local deletes are held for the user to decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldReason {
    RootEmpty,
    Bulk,
}

/// Deletes held back, and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldDeletes {
    pub reason: HeldReason,
    pub count: usize,
}

/// What the user chose for held deletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldChoice {
    Apply,
    Restore,
}

/// A file put back, or a path not placed, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteNotice {
    pub path: String,
    pub reason: String,
    pub at: DateTime<Utc>,
}

/// The whole index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncIndex {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default)]
    pub entries: BTreeMap<String, IndexEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictNotice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sent_for_review: Vec<SentForReview>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pending_deletes: BTreeMap<String, PendingDelete>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_deletes: Option<HeldDeletes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_choice: Option<HeldChoice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delete_notices: Vec<DeleteNotice>,
}

const NOTICES_KEPT: usize = 20;

impl SyncIndex {
    pub fn load(path: &Path) -> Result<Self, SyncError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            },
            Err(error) => return Err(SyncError::io("read the Drive index", path, error)),
        };
        match serde_json::from_slice(&bytes) {
            Ok(index) => Ok(index),
            Err(source) => {
                let quarantined = quarantine_path(path);
                std::fs::rename(path, &quarantined)
                    .map_err(|error| SyncError::io("move aside the Drive index", path, error))?;
                tracing::error!(path = %path.display(), quarantined = %quarantined.display(), "the Drive index did not decode");
                Err(SyncError::IndexCorrupt {
                    path: path.to_path_buf(),
                    quarantined,
                    source,
                })
            },
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), SyncError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(SyncError::Encode)?;
        write_atomic(path, &bytes, Access::Shared).map_err(SyncError::Secure)
    }

    #[must_use]
    pub fn path_of(&self, document_id: &DocumentId) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, entry)| entry.document_id.as_ref() == Some(document_id))
            .map(|(rel, _)| rel.as_str())
    }

    pub(crate) fn push_conflict(&mut self, notice: &ConflictNotice) {
        if !self
            .conflicts
            .iter()
            .any(|known| known.path == notice.path && known.sibling == notice.sibling)
        {
            self.conflicts.push(notice.clone());
        }
    }

    pub(crate) fn clear_unplaced(&mut self, document_id: &DocumentId) {
        self.conflicts
            .retain(|notice| !notice.unplaced || notice.document_id.as_ref() != Some(document_id));
    }

    #[must_use]
    pub(crate) fn has_unplaced(&self, document_id: &DocumentId) -> bool {
        self.conflicts
            .iter()
            .any(|notice| notice.unplaced && notice.document_id.as_ref() == Some(document_id))
    }

    pub fn record_notice(&mut self, rel: &str, reason: String) {
        if self
            .delete_notices
            .iter()
            .any(|notice| notice.path == rel && notice.reason == reason)
        {
            return;
        }
        self.delete_notices.push(DeleteNotice {
            path: rel.to_owned(),
            reason,
            at: Utc::now(),
        });
        let over = self.delete_notices.len().saturating_sub(NOTICES_KEPT);
        self.delete_notices.drain(..over);
    }
}

fn quarantine_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(std::ffi::OsString::from)
        .unwrap_or_default();
    name.push(format!(
        ".corrupt-{}",
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
    ));
    path.with_file_name(name)
}
