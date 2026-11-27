//! The options, targets and reports the snapshot store takes and returns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::SnapshotId;

const DEFAULT_MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 100_000;
const DEFAULT_MAX_AGE: Duration = Duration::from_hours(168);
const DEFAULT_KEEP_LATEST: usize = 100;

/// Limits applied while walking a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapshotOptions {
    pub max_file_size: u64,
    pub max_files: usize,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

/// The right-hand side of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffTarget {
    Snapshot(SnapshotId),
    Working,
}

/// A stored snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: SnapshotId,
    pub created: SystemTime,
    pub files: usize,
}

/// Which snapshots survive a prune: the newest `keep_latest`, and any younger
/// than `max_age`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrunePolicy {
    pub max_age: Duration,
    pub keep_latest: usize,
}

impl Default for PrunePolicy {
    fn default() -> Self {
        Self {
            max_age: DEFAULT_MAX_AGE,
            keep_latest: DEFAULT_KEEP_LATEST,
        }
    }
}

/// What a prune removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub snapshots_removed: usize,
    pub objects_removed: usize,
    pub bytes_freed: u64,
}
