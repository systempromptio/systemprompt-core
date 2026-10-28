//! What the Drive folder last agreed with the gateway on, persisted at
//! `<Drive>/.drive-state.json`, with the open proposals last fetched.
//!
//! A missing file is a first run. A file that does not decode is never
//! silently replaced: it is renamed aside to `.drive-state.json.corrupt-<ts>`
//! and the load fails, so the loss is reported before the next cycle starts
//! from an empty state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentProposalId, DocumentRevisionId, UserId,
};

use super::error::{DriveError, DriveResult};

/// The synced form of one file, keyed in [`DriveState::entries`] by its
/// `/`-separated path relative to the Drive folder.
///
/// `document_id` is absent for a file this device created until the change
/// feed reports the document the gateway made for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveEntryState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    pub revision_id: DocumentRevisionId,
    pub sha256: ContentHash,
    pub mtime_unix: i64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictNotice {
    pub path: String,
    pub sibling: String,
    pub head_revision_id: DocumentRevisionId,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenProposal {
    pub proposal_id: DocumentProposalId,
    pub document_id: DocumentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub proposer_id: UserId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default)]
    pub entries: BTreeMap<String, DriveEntryState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictNotice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<OpenProposal>,
}

impl DriveState {
    pub fn load(path: &Path) -> DriveResult<Self> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(DriveError::io("read drive state", path, e)),
        };
        match serde_json::from_slice(&bytes) {
            Ok(state) => Ok(state),
            Err(source) => {
                let quarantined = quarantine_path(path);
                std::fs::rename(path, &quarantined)
                    .map_err(|e| DriveError::io("quarantine drive state", path, e))?;
                Err(DriveError::StateCorrupt {
                    path: path.to_path_buf(),
                    quarantined,
                    source,
                })
            },
        }
    }

    pub fn save(&self, path: &Path) -> DriveResult<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(DriveError::Encode)?;
        crate::fsutil::atomic_write_0644(path, &bytes)
            .map_err(|e| DriveError::io("write drive state", path, e))
    }

    #[must_use]
    pub fn path_of(&self, document_id: &DocumentId) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, entry)| entry.document_id.as_ref() == Some(document_id))
            .map(|(rel, _)| rel.as_str())
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
