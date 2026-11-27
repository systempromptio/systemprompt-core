//! Applying the instance's view to the Drive folder.
//!
//! With no cursor the folder is reconciled against the whole manifest; after
//! that only the change feed since the cursor is read. The cursor and each
//! entry advance only once the file on disk matches them, and the index is
//! saved after every change, so an interrupted pull resumes where it stopped.
//! Placing a head and keeping a local edit beside it are [`super::place`]'s;
//! the feed is never trusted to name a file this device did not sync. A file
//! the user removed stays removed while [`super::deletes`] decides its fate. A
//! cursor answered `410` `cursor_reset` is dropped and the folder reconciled
//! against the manifest in the same pull.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use sp_protocol::ids::{DocumentId, DocumentRevisionId};
use sp_protocol::wire::drive::{ChangeKind, FeedChange, FeedManifest};

use super::cycle::{Cycle, Report};
use super::disk::{free_conflict_sibling, move_file, move_to_trash};
use super::error::SyncError;
use super::index::{ConflictNotice, SyncIndex};
use super::names::PathRefusal;
use super::place::{HeadRef, move_and_place, rename};
use super::scan::hash;
use super::status::ActivityKind;

pub(crate) async fn pull(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    report: &mut Report,
) -> Result<(), SyncError> {
    let index_path = cx.root.index_path();
    let Some(since) = index.cursor.clone() else {
        let manifest = cx.feed.manifest(cx.token).await?;
        return reconcile(cx, index, manifest, report).await;
    };
    let feed = match cx.feed.changes(cx.token, &since).await {
        Ok(feed) => feed,
        Err(error) if error.is_cursor_reset() => {
            tracing::warn!("the instance reset the Drive cursor; reading the whole manifest");
            index.cursor = None;
            index.save(&index_path)?;
            let manifest = cx.feed.manifest(cx.token).await?;
            return reconcile(cx, index, manifest, report).await;
        },
        Err(error) => return Err(error),
    };
    retry_unplaced(cx, index, report).await?;
    for change in &feed.changes {
        let applied = apply(cx, index, change, report).await;
        skip_if_unplaceable(
            index,
            applied,
            &change.document_id,
            change.revision_id.as_ref(),
            report,
        )?;
        index.cursor = Some(change.cursor.clone());
        index.save(&index_path)?;
    }
    index.cursor = Some(feed.next_cursor);
    index.save(&index_path)
}

async fn reconcile(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    manifest: FeedManifest,
    report: &mut Report,
) -> Result<(), SyncError> {
    for entry in manifest.entries {
        let head = HeadRef {
            document_id: entry.document_id,
            revision_id: entry.head_revision_id,
            sha256: entry.content_sha256,
        };
        let (document_id, revision_id) = (head.document_id.clone(), head.revision_id.clone());
        let placed = move_and_place(cx, index, &entry.path, head, report).await;
        skip_if_unplaceable(index, placed, &document_id, Some(&revision_id), report)?;
    }
    index.cursor = Some(manifest.cursor);
    index.save(&cx.root.index_path())
}

async fn apply(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    change: &FeedChange,
    report: &mut Report,
) -> Result<(), SyncError> {
    let known = index.path_of(&change.document_id).map(str::to_owned);
    match &change.kind {
        ChangeKind::Renamed { .. } if change.revision_id.is_none() => match known {
            Some(known) if !index.has_unplaced(&change.document_id) => {
                if known != change.path {
                    rename(cx, index, &known, &change.path, &change.document_id)?;
                    report.note(&change.path, ActivityKind::Moved { from: known });
                }
                index.clear_unplaced(&change.document_id);
                Ok(())
            },
            _ => place_from_manifest(cx, index, &change.document_id, report).await,
        },
        ChangeKind::Created | ChangeKind::Updated | ChangeKind::Renamed { .. } => {
            let head = head_of(change)?;
            move_and_place(cx, index, &change.path, head, report).await
        },
        ChangeKind::Deleted => remote_delete(cx, index, &change.document_id, report),
    }
}

fn head_of(change: &FeedChange) -> Result<HeadRef, SyncError> {
    let incomplete = || SyncError::IncompleteChange {
        path: change.path.clone(),
    };
    Ok(HeadRef {
        document_id: change.document_id.clone(),
        revision_id: change.revision_id.clone().ok_or_else(incomplete)?,
        sha256: change.content_sha256.clone().ok_or_else(incomplete)?,
    })
}

fn remote_delete(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    document_id: &DocumentId,
    report: &mut Report,
) -> Result<(), SyncError> {
    index.clear_unplaced(document_id);
    let Some(rel) = index.path_of(document_id).map(str::to_owned) else {
        tracing::debug!("the Drive sync ignored the deletion of a document it never placed");
        return Ok(());
    };
    let Some(synced) = index.entries.get(&rel).cloned() else {
        return Ok(());
    };
    let path = cx.root.file(&rel)?;
    if path.exists() && hash(&path)? != synced.sha256 {
        let sibling = free_conflict_sibling(cx.root, &rel, cx.host)?;
        move_file(cx.root, &path, &cx.root.file(&sibling)?)?;
        let notice = ConflictNotice {
            document_id: Some(document_id.clone()),
            unplaced: false,
            path: rel.clone(),
            sibling: sibling.clone(),
            head_revision_id: synced.revision_id,
            detected_at: Utc::now(),
        };
        index.push_conflict(&notice);
        tracing::warn!(path = %rel, sibling = %sibling, "the Drive sync kept a local edit to a document deleted remotely");
        report.note(&rel, ActivityKind::Conflict { kept_as: sibling });
    } else if let Some(trashed) = move_to_trash(cx.root, &rel, Utc::now())? {
        tracing::info!(path = %rel, trashed = %trashed.display(), "the Drive sync moved a deleted document to the trash");
        report.note(&rel, ActivityKind::DeletedRemotely);
    }
    index.entries.remove(&rel);
    index.pending_deletes.remove(&rel);
    Ok(())
}

async fn retry_unplaced(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    report: &mut Report,
) -> Result<(), SyncError> {
    let pending: Vec<DocumentId> = index
        .conflicts
        .iter()
        .filter(|notice| notice.unplaced)
        .filter_map(|notice| notice.document_id.clone())
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    let manifest = cx.feed.manifest(cx.token).await?;
    for document_id in &pending {
        place_listed(cx, index, &manifest, document_id, report).await?;
    }
    index.save(&cx.root.index_path())
}

async fn place_from_manifest(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    document_id: &DocumentId,
    report: &mut Report,
) -> Result<(), SyncError> {
    let manifest = cx.feed.manifest(cx.token).await?;
    place_listed(cx, index, &manifest, document_id, report).await
}

async fn place_listed(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    manifest: &FeedManifest,
    document_id: &DocumentId,
    report: &mut Report,
) -> Result<(), SyncError> {
    let Some(entry) = manifest
        .entries
        .iter()
        .find(|entry| &entry.document_id == document_id)
    else {
        tracing::debug!("an unplaced document has no head; its deletion will clear it");
        return Ok(());
    };
    let head = HeadRef {
        document_id: entry.document_id.clone(),
        revision_id: entry.head_revision_id.clone(),
        sha256: entry.content_sha256.clone(),
    };
    let placed = move_and_place(cx, index, &entry.path, head, report).await;
    skip_if_unplaceable(
        index,
        placed,
        document_id,
        Some(&entry.head_revision_id),
        report,
    )
}

fn skip_if_unplaceable(
    index: &mut SyncIndex,
    result: Result<(), SyncError>,
    document_id: &DocumentId,
    revision_id: Option<&DocumentRevisionId>,
    report: &mut Report,
) -> Result<(), SyncError> {
    match result {
        Ok(()) => Ok(()),
        Err(SyncError::Unplaceable { path, refusal }) => {
            tracing::warn!(path = %path, refusal = %refusal, "the Drive sync did not place a document");
            let revision_id = revision_id.cloned().or_else(|| {
                index
                    .path_of(document_id)
                    .and_then(|known| index.entries.get(known))
                    .map(|entry| entry.revision_id.clone())
            });
            let Some(revision_id) = revision_id else {
                index.record_notice(&path, format!("not placed: {refusal}"));
                return Ok(());
            };
            let sibling = match &refusal {
                PathRefusal::CaseCollision { existing } => existing.clone(),
                _ => path.clone(),
            };
            index.clear_unplaced(document_id);
            let notice = ConflictNotice {
                document_id: Some(document_id.clone()),
                unplaced: true,
                path: path.clone(),
                sibling,
                head_revision_id: revision_id,
                detected_at: Utc::now(),
            };
            index.push_conflict(&notice);
            report.note(
                &path,
                ActivityKind::NotPlaced {
                    reason: refusal.to_string(),
                },
            );
            Ok(())
        },
        Err(error) => Err(error),
    }
}
