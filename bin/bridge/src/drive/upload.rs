//! Committing local edits as document revisions.
//!
//! Each changed file is read once, hashed from the bytes actually sent, and
//! committed with the revision it was based on as `If-Match`. A committed
//! revision becomes the file's state entry. A merged revision (the server
//! landed the edit on a newer head) is recorded against the uploaded bytes,
//! so the next pull replaces the file with the merged content; a conflict
//! hands the file to [`super::conflict`]. A proposed edit (the device may
//! only suggest on that document) is a success, not an error: the local file
//! is kept and recorded as sent, so it is not uploaded again, and the user
//! is told it went for review. A file that changed again while it was
//! uploading is left for the next cycle rather than recorded as synced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_models::documents::{DriveCommitMeta, DriveCommitResponse};

use super::error::{DriveError, DriveResult};
use super::hash::hash_bytes;
use super::scan::{LocalChange, local_changes, metadata};
use super::state::{ConflictNotice, DriveEntryState, DriveState, SentForReview};
use super::{Cycle, conflict};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UploadReport {
    pub committed: usize,
    pub merged: usize,
    pub conflicts: Vec<ConflictNotice>,
    pub deferred: usize,
    pub sent_for_review: Vec<SentForReview>,
}

pub async fn upload(cycle: Cycle<'_>, state: &mut DriveState) -> DriveResult<UploadReport> {
    let mut report = UploadReport::default();
    for change in local_changes(cycle.root, state)? {
        upload_one(cycle, state, change, &mut report).await?;
    }
    Ok(report)
}

pub async fn upload_one(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    change: LocalChange,
    report: &mut UploadReport,
) -> DriveResult<()> {
    let path = &change.file.abs;
    let bytes = std::fs::read(path).map_err(|e| DriveError::io("read for upload", path, e))?;
    let sha256 = hash_bytes(&bytes);
    let meta = DriveCommitMeta {
        document_id: change.base.as_ref().and_then(|b| b.document_id.clone()),
        path: change.file.rel.clone(),
        base_revision: change.base.as_ref().map(|b| b.revision_id.clone()),
        content_sha256: sha256.clone(),
        size_bytes: bytes.len() as u64,
        device_id: cycle.device_id.clone(),
        message: None,
    };
    let size = meta.size_bytes;
    match cycle.client.commit(cycle.bearer, &meta, bytes).await? {
        DriveCommitResponse::Committed { revision_id } => {
            let (mtime_unix, now_size) = metadata(path)?;
            let unchanged = now_size == size && super::hash::hash_file(path)?.0 == sha256;
            if !unchanged {
                report.deferred += 1;
                tracing::info!(path = %change.file.rel, "drive file changed during upload; next cycle commits it");
            }
            let entry = DriveEntryState {
                document_id: meta.document_id,
                revision_id,
                sha256,
                mtime_unix: if unchanged { mtime_unix } else { 0 },
                size,
            };
            state.entries.insert(change.file.rel, entry);
            state.save(&cycle.root.state_path())?;
            report.committed += 1;
        },
        DriveCommitResponse::Merged { revision_id, .. } => {
            let (mtime_unix, now_size) = metadata(path)?;
            let unchanged = now_size == size && super::hash::hash_file(path)?.0 == sha256;
            let entry = DriveEntryState {
                document_id: meta.document_id,
                revision_id,
                sha256,
                mtime_unix: if unchanged { mtime_unix } else { 0 },
                size,
            };
            state.entries.insert(change.file.rel, entry);
            state.save(&cycle.root.state_path())?;
            report.merged += 1;
        },
        DriveCommitResponse::Proposed {
            proposal_id,
            revision_id,
        } => {
            let (mtime_unix, _) = metadata(path)?;
            let entry = DriveEntryState {
                document_id: meta.document_id,
                revision_id: meta.base_revision.unwrap_or(revision_id),
                sha256,
                mtime_unix,
                size,
            };
            let notice = SentForReview {
                path: change.file.rel.clone(),
                proposal_id,
                sent_at: Utc::now(),
            };
            tracing::info!(
                path = %notice.path,
                proposal_id = %notice.proposal_id,
                "drive edit sent for review; the local file is kept"
            );
            state.entries.insert(change.file.rel, entry);
            state
                .sent_for_review
                .retain(|sent| sent.path != notice.path);
            state.sent_for_review.push(notice.clone());
            state.save(&cycle.root.state_path())?;
            report.sent_for_review.push(notice);
        },
        DriveCommitResponse::Conflict {
            head_revision_id, ..
        } => {
            let notice =
                conflict::resolve(cycle, state, &change.file.rel, &head_revision_id).await?;
            report.conflicts.push(notice);
        },
    }
    Ok(())
}
