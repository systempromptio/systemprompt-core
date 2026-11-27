//! A file the user removed from the Drive folder, carried to the instance.
//!
//! A cycle first [`observe`]s which synced files are absent, before the pull,
//! so the pull neither restores them nor takes them for gone. After the pull,
//! [`apply`] acts on them:
//!
//! - A new local file with the digest of exactly one absent file is that file
//!   renamed: the document moves and keeps its history. A move the instance
//!   refuses puts the file back under its old name.
//! - An absent file is trashed only once it has been absent for
//!   [`CONFIRM_CYCLES`] cycles and [`CONFIRM_SECS`], because editors save by
//!   deleting and re-creating.
//! - A missing or emptied folder, or more absences than a person plausibly
//!   meant ([`is_bulk`]), trashes nothing: the deletes are held until the user
//!   applies or restores them.
//! - The trash names the revision last synced; a newer head, or a refusal,
//!   restores the head with a notice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use sp_protocol::ids::DocumentId;
use sp_protocol::wire::drive::TrashOutcome;

use super::cycle::{Cycle, Report};
use super::disk::move_file;
use super::error::SyncError;
use super::index::{HeldChoice, HeldDeletes, HeldReason, IndexEntry, PendingDelete, SyncIndex};
use super::restore::{restore_all, restore_one};
use super::root::SyncRoot;
use super::scan::{LocalChange, local_changes, metadata, walk};
use super::status::ActivityKind;

pub(crate) const CONFIRM_CYCLES: u32 = 2;
pub(crate) const CONFIRM_SECS: i64 = 15;
pub(crate) const BULK_ABSOLUTE: usize = 10;
pub(crate) const BULK_SMALL_FOLDER: usize = 3;

pub(crate) fn observe(root: &SyncRoot, index: &mut SyncIndex, now: DateTime<Utc>) -> bool {
    let before = index.pending_deletes.clone();
    let absent: Vec<String> = index
        .entries
        .keys()
        .filter(|rel| root.file(rel).is_ok_and(|path| !path.exists()))
        .cloned()
        .collect();
    index.pending_deletes.retain(|rel, _| absent.contains(rel));
    for rel in absent {
        index
            .pending_deletes
            .entry(rel)
            .and_modify(|pending| pending.cycles = pending.cycles.saturating_add(1))
            .or_insert(PendingDelete {
                noticed_at: now,
                cycles: 1,
            });
    }
    if index.pending_deletes.is_empty() {
        index.held_deletes = None;
        index.held_choice = None;
    }
    before != index.pending_deletes
}

#[must_use]
pub(crate) const fn is_bulk(absent: usize, synced: usize) -> bool {
    absent > BULK_ABSOLUTE || (absent > BULK_SMALL_FOLDER && absent.saturating_mul(2) > synced)
}

#[must_use]
pub(crate) fn is_confirmed(pending: &PendingDelete, now: DateTime<Utc>) -> bool {
    pending.cycles >= CONFIRM_CYCLES && now - pending.noticed_at >= Duration::seconds(CONFIRM_SECS)
}

fn root_looks_lost(root: &SyncRoot, index: &SyncIndex) -> Result<bool, SyncError> {
    if !root.dir().is_dir() {
        return Ok(true);
    }
    Ok(index.entries.len() > 1 && walk(root)?.is_empty())
}

pub(crate) async fn apply(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    report: &mut Report,
) -> Result<(), SyncError> {
    let index_path = cx.root.index_path();
    if index.pending_deletes.is_empty() {
        return Ok(());
    }
    let lost = root_looks_lost(cx.root, index)?;
    let mut known = KnownIds::default();
    if !lost {
        apply_moves(cx, index, &mut known, report).await?;
    }
    let absent = index.pending_deletes.len();
    if absent == 0 {
        return index.save(&index_path);
    }
    let choice = index.held_choice.take();
    if choice == Some(HeldChoice::Restore) {
        restore_all(cx, index, report).await?;
        index.held_deletes = None;
        return index.save(&index_path);
    }
    let reason = if lost {
        Some(HeldReason::RootEmpty)
    } else if is_bulk(absent, index.entries.len()) {
        Some(HeldReason::Bulk)
    } else {
        None
    };
    if let (Some(reason), None) = (reason, choice) {
        tracing::warn!(
            ?reason,
            count = absent,
            "the Drive sync is holding local deletes for the user"
        );
        index.held_deletes = Some(HeldDeletes {
            reason,
            count: absent,
        });
        return index.save(&index_path);
    }
    index.held_deletes = None;
    let now = Utc::now();
    let due: Vec<String> = index
        .pending_deletes
        .iter()
        .filter(|(_, pending)| choice.is_some() || is_confirmed(pending, now))
        .map(|(rel, _)| rel.clone())
        .collect();
    for rel in due {
        trash_one(cx, index, &rel, &mut known, report).await?;
        index.save(&index_path)?;
    }
    index.save(&index_path)
}

#[derive(Default)]
struct KnownIds(Option<BTreeMap<String, DocumentId>>);

impl KnownIds {
    async fn resolve(
        &mut self,
        cx: Cycle<'_>,
        rel: &str,
        entry: &IndexEntry,
    ) -> Result<Option<DocumentId>, SyncError> {
        if let Some(id) = &entry.document_id {
            return Ok(Some(id.clone()));
        }
        if self.0.is_none() {
            let manifest = cx.feed.manifest(cx.token).await?;
            self.0 = Some(
                manifest
                    .entries
                    .into_iter()
                    .map(|entry| (entry.path, entry.document_id))
                    .collect(),
            );
        }
        Ok(self.0.as_ref().and_then(|ids| ids.get(rel).cloned()))
    }
}

fn unique_rename<'a>(
    entry: &IndexEntry,
    pending: &BTreeMap<String, PendingDelete>,
    entries: &BTreeMap<String, IndexEntry>,
    created: &'a [LocalChange],
) -> Option<&'a LocalChange> {
    let twins = pending
        .keys()
        .filter_map(|rel| entries.get(rel))
        .filter(|other| other.sha256 == entry.sha256)
        .count();
    let mut same = created
        .iter()
        .filter(|change| change.sha256 == entry.sha256);
    let only = same.next()?;
    (twins == 1 && same.next().is_none()).then_some(only)
}

async fn apply_moves(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    known: &mut KnownIds,
    report: &mut Report,
) -> Result<(), SyncError> {
    let created: Vec<LocalChange> = local_changes(cx.root, index)?
        .into_iter()
        .filter(|change| change.base.is_none())
        .collect();
    let rels: Vec<String> = index.pending_deletes.keys().cloned().collect();
    for rel in rels {
        let Some(entry) = index.entries.get(&rel).cloned() else {
            continue;
        };
        let Some(change) =
            unique_rename(&entry, &index.pending_deletes, &index.entries, &created).cloned()
        else {
            continue;
        };
        let Some(document_id) = known.resolve(cx, &rel, &entry).await? else {
            continue;
        };
        match cx
            .feed
            .move_document(cx.token, &document_id, &change.file.rel)
            .await
        {
            Ok(()) => {
                let (mtime_unix, size) = metadata(&change.file.abs)?;
                index.entries.remove(&rel);
                index.entries.insert(
                    change.file.rel.clone(),
                    IndexEntry {
                        document_id: Some(document_id),
                        mtime_unix,
                        size,
                        ..entry
                    },
                );
                index.pending_deletes.remove(&rel);
                report.note(&change.file.rel, ActivityKind::Moved { from: rel.clone() });
            },
            Err(error) if error.is_refusal() => {
                move_file(cx.root, &change.file.abs, &cx.root.file(&rel)?)?;
                index.pending_deletes.remove(&rel);
                index.record_notice(&rel, format!("the move was refused: {error}"));
                report.note(&rel, ActivityKind::Restored);
            },
            Err(error) => return Err(error),
        }
        index.save(&cx.root.index_path())?;
    }
    Ok(())
}

async fn trash_one(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    known: &mut KnownIds,
    report: &mut Report,
) -> Result<(), SyncError> {
    let Some(entry) = index.entries.get(rel).cloned() else {
        index.pending_deletes.remove(rel);
        return Ok(());
    };
    let Some(document_id) = known.resolve(cx, rel, &entry).await? else {
        index.entries.remove(rel);
        index.pending_deletes.remove(rel);
        return Ok(());
    };
    match cx
        .feed
        .trash(cx.token, &document_id, &entry.revision_id)
        .await
    {
        Ok(TrashOutcome::Trashed) => {
            index.entries.remove(rel);
            index.pending_deletes.remove(rel);
            tracing::info!(path = %rel, "the Drive sync moved a locally deleted document to the instance's trash");
            report.note(rel, ActivityKind::Trashed);
        },
        Ok(TrashOutcome::Conflict { .. }) => {
            restore_one(cx, index, rel, &document_id, report).await?;
            index.record_notice(
                rel,
                "it changed on the instance, so it was not deleted".to_owned(),
            );
        },
        Err(error) if error.is_gone() => {
            index.entries.remove(rel);
            index.pending_deletes.remove(rel);
        },
        Err(error) if error.is_refusal() => {
            restore_one(cx, index, rel, &document_id, report).await?;
            index.record_notice(rel, format!("the delete was refused: {error}"));
        },
        Err(error) => return Err(error),
    }
    Ok(())
}
