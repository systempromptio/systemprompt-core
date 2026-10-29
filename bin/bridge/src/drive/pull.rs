//! Applying the gateway's view to the Drive folder.
//!
//! With no cursor the folder is reconciled against the full manifest; after
//! that only `changes?since=<cursor>` is read. The cursor and each entry
//! advance only after the file on disk matches them, and the state is saved
//! after every applied change, so an interrupted pull resumes where it
//! stopped. A local file with edits the state does not know of is never
//! overwritten: it is kept as a conflict sibling first. The local file is
//! hashed again just before the downloaded head replaces it, so an edit made
//! while the blob was downloading is kept beside too.
//!
//! A cursor the gateway answers with `410 Gone` and error key `cursor_reset`
//! is no longer a feed position: it is dropped, saved as dropped, and the
//! folder is reconciled against the full manifest in the same pull.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::{DriveChange, DriveChangeKind, DriveManifest};

use super::Cycle;
use super::conflict::{HeadRef, keep_local_beside};
use super::error::{DriveError, DriveResult};
use super::hash::hash_file;
use super::state::{ConflictNotice, DriveState};
use super::write::{GuardedWrite, move_file, move_to_trash, synced_entry, write_unless_changed};

const CURSOR_RESET: &str = "cursor_reset";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PullReport {
    pub applied: usize,
    pub conflicts: Vec<ConflictNotice>,
}

pub async fn pull(cycle: Cycle<'_>, state: &mut DriveState) -> DriveResult<PullReport> {
    let Some(since) = state.cursor.clone() else {
        let manifest = cycle.client.manifest(cycle.bearer).await?;
        return reconcile(cycle, state, manifest).await;
    };
    let feed = match cycle.client.changes(cycle.bearer, &since).await {
        Ok(feed) => feed,
        Err(error) if is_cursor_reset(&error) => {
            tracing::warn!(cursor = %since, "drive cursor reset by the gateway; re-reading the manifest");
            state.cursor = None;
            state.save(&cycle.root.state_path())?;
            let manifest = cycle.client.manifest(cycle.bearer).await?;
            return reconcile(cycle, state, manifest).await;
        },
        Err(error) => return Err(error),
    };
    let mut report = PullReport::default();
    for change in &feed.changes {
        apply(cycle, state, change, &mut report).await?;
        state.cursor = Some(change.cursor.clone());
        state.save(&cycle.root.state_path())?;
    }
    state.cursor = Some(feed.next_cursor);
    state.save(&cycle.root.state_path())?;
    Ok(report)
}

fn is_cursor_reset(error: &DriveError) -> bool {
    matches!(
        error,
        DriveError::Rejected { status, rejection, .. }
            if *status == reqwest::StatusCode::GONE
                && rejection.error_key.as_deref() == Some(CURSOR_RESET)
    )
}

pub async fn reconcile(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    manifest: DriveManifest,
) -> DriveResult<PullReport> {
    let mut report = PullReport::default();
    for entry in manifest.entries {
        let head = HeadRef {
            document_id: entry.document_id,
            revision_id: entry.head_revision_id,
            sha256: entry.content_sha256,
        };
        place_head(cycle, state, &entry.path, head, &mut report).await?;
    }
    state.cursor = Some(manifest.cursor);
    state.save(&cycle.root.state_path())?;
    Ok(report)
}

async fn apply(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    change: &DriveChange,
    report: &mut PullReport,
) -> DriveResult<()> {
    match &change.kind {
        DriveChangeKind::Created | DriveChangeKind::Updated => {
            let head = head_of(change)?;
            if let Some(known) = state.path_of(&change.document_id).map(str::to_owned)
                && known != change.path
            {
                rename(cycle, state, &known, &change.path, &change.document_id)?;
            }
            place_head(cycle, state, &change.path, head, report).await
        },
        DriveChangeKind::Renamed { from } => {
            rename(cycle, state, from, &change.path, &change.document_id)?;
            if change.revision_id.is_some() {
                let head = head_of(change)?;
                place_head(cycle, state, &change.path, head, report).await?;
            }
            Ok(())
        },
        DriveChangeKind::Deleted => {
            let rel = state
                .path_of(&change.document_id)
                .map_or_else(|| change.path.clone(), str::to_owned);
            if let Some(trashed) = move_to_trash(cycle.root, &rel)? {
                tracing::info!(path = %rel, trashed = %trashed.display(), "drive moved a deleted document to the trash");
            }
            state.entries.remove(&rel);
            report.applied += 1;
            Ok(())
        },
    }
}

fn head_of(change: &DriveChange) -> DriveResult<HeadRef> {
    let incomplete = || DriveError::IncompleteChange {
        document_id: change.document_id.clone(),
        path: change.path.clone(),
    };
    Ok(HeadRef {
        document_id: change.document_id.clone(),
        revision_id: change.revision_id.clone().ok_or_else(incomplete)?,
        sha256: change.content_sha256.clone().ok_or_else(incomplete)?,
    })
}

async fn place_head(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    head: HeadRef,
    report: &mut PullReport,
) -> DriveResult<()> {
    let path = cycle.root.file(rel)?;
    let known = state.entries.get(rel).cloned();
    if known
        .as_ref()
        .is_some_and(|e| e.revision_id == head.revision_id && e.sha256 == head.sha256)
        && path.exists()
    {
        return Ok(());
    }
    let local = if path.exists() {
        Some(hash_file(&path)?.0)
    } else {
        None
    };
    if local.as_ref() == Some(&head.sha256) {
        adopt(state, &path, rel, head)?;
        report.applied += 1;
        return Ok(());
    }
    let bytes = cycle
        .client
        .download_blob(cycle.bearer, &head.sha256)
        .await?;
    let unsynced_edit = local
        .as_ref()
        .is_some_and(|sha| known.as_ref().is_none_or(|e| &e.sha256 != sha));
    let written = !unsynced_edit
        && write_unless_changed(&path, &bytes, &head.sha256, local.as_ref())?
            == GuardedWrite::Written;
    if written {
        adopt(state, &path, rel, head)?;
    } else {
        let notice = keep_local_beside(cycle, state, rel, &bytes, head)?;
        report.conflicts.push(notice);
    }
    report.applied += 1;
    Ok(())
}

fn adopt(
    state: &mut DriveState,
    path: &std::path::Path,
    rel: &str,
    head: HeadRef,
) -> DriveResult<()> {
    let HeadRef {
        document_id,
        revision_id,
        sha256,
    } = head;
    let entry = synced_entry(path, Some(document_id), revision_id, sha256)?;
    state.entries.insert(rel.to_owned(), entry);
    Ok(())
}

fn rename(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    from: &str,
    to: &str,
    document_id: &DocumentId,
) -> DriveResult<()> {
    let source = cycle.root.file(from)?;
    let target = cycle.root.file(to)?;
    if source.exists() && !target.exists() {
        move_file(&source, &target)?;
    }
    if let Some(mut entry) = state.entries.remove(from) {
        entry.document_id = Some(document_id.clone());
        if target.exists() {
            let (mtime_unix, size) = super::scan::metadata(&target)?;
            entry.mtime_unix = mtime_unix;
            entry.size = size;
        }
        state.entries.insert(to.to_owned(), entry);
    }
    Ok(())
}
