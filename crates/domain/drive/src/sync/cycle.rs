//! One sync cycle: what it runs against, what it reports, and the order of its
//! steps.
//!
//! A cycle empties expired trash, drops conflicts the user resolved, notes the
//! synced files gone from the folder, pulls the instance's changes, carries
//! local deletes and renames over, then uploads local edits, so an edit is
//! always committed on top of the newest head this device knows. Cycles never
//! overlap: the caller holds a lock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_device::protocol::Jwt;
use systemprompt_identifiers::DriveDeviceId;

use super::deletes;
use super::disk::prune_trash;
use super::error::SyncError;
use super::feed::FeedClient;
use super::index::{ConflictNotice, DeleteNotice, HeldDeletes, SentForReview, SyncIndex};
use super::pull::pull;
use super::push::upload;
use super::root::SyncRoot;
use super::scan::local_changes;
use super::status::{ActivityItem, ActivityKind};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Cycle<'a> {
    pub(crate) feed: &'a FeedClient,
    pub(crate) token: &'a Jwt,
    pub(crate) root: &'a SyncRoot,
    pub(crate) device_id: &'a DriveDeviceId,
    pub(crate) host: &'a str,
}

#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) activity: Vec<ActivityItem>,
    pub(crate) refused: Vec<(String, String)>,
}

impl Report {
    pub(crate) fn note(&mut self, path: &str, kind: ActivityKind) {
        self.activity.push(ActivityItem {
            at: Utc::now(),
            path: path.to_owned(),
            kind,
        });
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Summary {
    pub(crate) synced: usize,
    pub(crate) pending_uploads: usize,
    pub(crate) conflicts: Vec<ConflictNotice>,
    pub(crate) sent_for_review: Vec<SentForReview>,
    pub(crate) held_deletes: Option<HeldDeletes>,
    pub(crate) notices: Vec<DeleteNotice>,
}

impl Summary {
    pub(crate) fn of(index: &SyncIndex, pending_uploads: usize) -> Self {
        Self {
            synced: index.entries.len(),
            pending_uploads,
            conflicts: index.conflicts.clone(),
            sent_for_review: index.sent_for_review.clone(),
            held_deletes: index.held_deletes,
            notices: index.delete_notices.clone(),
        }
    }
}

pub(crate) async fn run(cx: Cycle<'_>, report: &mut Report) -> Result<Summary, SyncError> {
    let index_path = cx.root.index_path();
    let mut index = SyncIndex::load(&index_path)?;
    let pruned = prune_trash(cx.root, Utc::now())?;
    if pruned > 0 {
        tracing::info!(stamps = pruned, "the Drive sync emptied expired trash");
    }
    let resolved = prune_resolved(cx.root, &mut index);
    let observed = deletes::observe(cx.root, &mut index, Utc::now());
    if resolved || observed {
        index.save(&index_path)?;
    }
    pull(cx, &mut index, report).await?;
    deletes::apply(cx, &mut index, report).await?;
    upload(cx, &mut index, report).await?;
    let pending = local_changes(cx.root, &index)?.len();
    Ok(Summary::of(&index, pending))
}

pub(crate) fn read_summary(root: &SyncRoot) -> Result<Summary, SyncError> {
    let mut index = SyncIndex::load(&root.index_path())?;
    prune_resolved(root, &mut index);
    let pending = local_changes(root, &index)?.len();
    Ok(Summary::of(&index, pending))
}

fn prune_resolved(root: &SyncRoot, index: &mut SyncIndex) -> bool {
    let before = index.conflicts.len();
    index.conflicts.retain(|notice| {
        notice.unplaced || root.file(&notice.sibling).is_ok_and(|path| path.exists())
    });
    before != index.conflicts.len()
}
