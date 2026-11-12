//! One Drive sync cycle and the first-run check.
//!
//! A cycle removes expired trash, notes the files gone from the folder, pulls
//! the server's changes, carries the local deletes and renames over, pushes
//! local edits, refreshes the open proposals and folds the outcome into the
//! shared status. The first-run check creates the Drive folder once the server
//! has documents for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;

use super::conflict::prune_resolved;
use super::deletes::{self, DeleteReport};
use super::error::{DriveError, DriveResult};
use super::pull::{PullReport, pull};
use super::scan::local_changes;
use super::state::DriveState;
use super::status::DriveHandle;
use super::upload::{UploadReport, upload};
use super::{Cycle, shares};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CycleReport {
    pub pull: PullReport,
    pub deletes: DeleteReport,
    pub upload: UploadReport,
    pub pending_uploads: usize,
}

pub async fn sync_once(cycle: Cycle<'_>, handle: &DriveHandle) -> DriveResult<CycleReport> {
    let _serialised = handle.cycle.lock().await;
    let state_path = cycle.root.state_path();
    let mut state = DriveState::load(&state_path)?;
    let pruned = super::trash::prune(cycle.root, Utc::now())?;
    if pruned > 0 {
        tracing::info!(stamps = pruned, "drive removed expired trash folders");
    }
    if prune_resolved(cycle.root, &mut state) | shares::record(&mut state, handle.take_shares()) {
        state.save(&state_path)?;
    }
    if deletes::observe(cycle.root, &mut state, Utc::now()) {
        state.save(&state_path)?;
    }
    let pull = pull(cycle, &mut state).await?;
    if shares::prune_synced(&mut state) {
        state.save(&state_path)?;
    }
    handle.mark_enabled(cycle.root);
    let deletes = deletes::apply(cycle, &mut state).await?;
    let upload = upload(cycle, &mut state).await?;
    if super::proposals::refresh(cycle, &mut state).await {
        state.save(&state_path)?;
    }
    let pending_uploads = local_changes(cycle.root, &state)?.len();
    let report = CycleReport {
        pull,
        deletes,
        upload,
        pending_uploads,
    };
    handle.update(|status| {
        status.synced_at = Some(Utc::now());
        status.pending_uploads = report.pending_uploads;
        status.conflicts.clone_from(&state.conflicts);
        status.proposals.clone_from(&state.proposals);
        status.sent_for_review.clone_from(&state.sent_for_review);
        status.shares.clone_from(&state.shares);
        status.held_deletes.clone_from(&state.held_deletes);
        status.delete_notices.clone_from(&state.delete_notices);
        status.last_error = report
            .upload
            .refused
            .first()
            .map(|refused| format!("{} was not uploaded: {}", refused.path, refused.reason));
    });
    Ok(report)
}

pub async fn ensure_enabled(cycle: Cycle<'_>, handle: &DriveHandle) -> DriveResult<bool> {
    let dir = cycle.root.dir();
    if !dir.is_dir() {
        let manifest = match cycle.client.manifest(cycle.bearer).await {
            Ok(manifest) => manifest,
            Err(DriveError::ServerDisabled { .. }) => {
                handle.mark_disabled();
                return Ok(false);
            },
            Err(error) => return Err(error),
        };
        if manifest.entries.is_empty() {
            return Ok(false);
        }
        std::fs::create_dir_all(dir).map_err(|e| DriveError::io("create Drive folder", dir, e))?;
        tracing::info!(root = %dir.display(), documents = manifest.entries.len(), "drive folder created");
        handle.mark_enabled(cycle.root);
    }
    Ok(true)
}
