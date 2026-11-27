//! What the shell reads about the Drive sync: its phase, counts, conflicts,
//! held deletes and recent activity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use super::index::{ConflictNotice, DeleteNotice, HeldDeletes, SentForReview};

/// Where the sync stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SyncPhase {
    #[default]
    Off,
    Idle,
    Syncing,
    Paused,
    Disabled,
    Error,
}

/// Why the last cycle failed, in the words the shell picks its message by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncProblemKind {
    SignedOut,
    NoDeviceToken,
    Unreachable,
    Refused,
    LocalFiles,
    Failed,
}

/// The last failure: its kind and a sentence with no secret in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProblem {
    pub kind: SyncProblemKind,
    pub message: String,
    pub at: DateTime<Utc>,
}

/// Counts for the status line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DriveCounts {
    pub synced: usize,
    pub pending_uploads: usize,
    pub conflicts: usize,
    pub held_deletes: usize,
}

/// One thing the sync did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityItem {
    pub at: DateTime<Utc>,
    pub path: String,
    pub kind: ActivityKind,
}

/// What happened to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityKind {
    Downloaded,
    Uploaded,
    Merged,
    SentForReview,
    Conflict { kept_as: String },
    Moved { from: String },
    Trashed,
    DeletedRemotely,
    Restored,
    Refused { reason: String },
    NotPlaced { reason: String },
}

/// The whole status, published on every change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriveStatus {
    pub phase: SyncPhase,
    pub root: Option<PathBuf>,
    pub last_synced: Option<DateTime<Utc>>,
    pub counts: DriveCounts,
    pub conflicts: Vec<ConflictNotice>,
    pub sent_for_review: Vec<SentForReview>,
    pub held_deletes: Option<HeldDeletes>,
    pub notices: Vec<DeleteNotice>,
    pub recent: Vec<ActivityItem>,
    pub problem: Option<SyncProblem>,
}

impl DriveStatus {
    pub(crate) fn absorb(&mut self, summary: super::cycle::Summary) {
        self.counts = DriveCounts {
            synced: summary.synced,
            pending_uploads: summary.pending_uploads,
            conflicts: summary.conflicts.len(),
            held_deletes: summary.held_deletes.map_or(0, |held| held.count),
        };
        self.conflicts = summary.conflicts;
        self.sent_for_review = summary.sent_for_review;
        self.held_deletes = summary.held_deletes;
        self.notices = summary.notices;
    }

    pub(crate) fn record(&mut self, activity: Vec<ActivityItem>, keep: usize) {
        for item in activity.into_iter().rev() {
            self.recent.insert(0, item);
        }
        self.recent.truncate(keep);
    }
}
