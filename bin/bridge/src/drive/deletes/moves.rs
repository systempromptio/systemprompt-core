//! A new file holding the bytes of exactly one absent file is that file
//! renamed: the document moves. A refused move puts the file back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentId;

use super::{DeleteReport, PendingDelete, is_refusal, notify};
use crate::drive::Cycle;
use crate::drive::error::DriveResult;
use crate::drive::scan::{LocalChange, local_changes, metadata};
use crate::drive::state::{DriveEntryState, DriveState};
use crate::drive::write::move_file;

/// The manifest, read at most once, for entries this device created and so
/// holds no document id for.
#[derive(Default)]
pub(super) struct KnownIds(Option<BTreeMap<String, DocumentId>>);

impl KnownIds {
    pub(super) async fn resolve(
        &mut self,
        cycle: Cycle<'_>,
        rel: &str,
        entry: &DriveEntryState,
    ) -> DriveResult<Option<DocumentId>> {
        if let Some(id) = &entry.document_id {
            return Ok(Some(id.clone()));
        }
        if self.0.is_none() {
            let manifest = cycle.client.manifest(cycle.bearer).await?;
            self.0 = Some(
                manifest
                    .entries
                    .into_iter()
                    .map(|e| (e.path, e.document_id))
                    .collect(),
            );
        }
        Ok(self.0.as_ref().and_then(|ids| ids.get(rel).cloned()))
    }
}

fn unique_rename<'a>(
    entry: &DriveEntryState,
    pending: &BTreeMap<String, PendingDelete>,
    entries: &BTreeMap<String, DriveEntryState>,
    created: &'a [LocalChange],
) -> Option<&'a LocalChange> {
    let twins = pending
        .keys()
        .filter_map(|rel| entries.get(rel))
        .filter(|other| other.sha256 == entry.sha256)
        .count();
    let mut same = created.iter().filter(|c| c.sha256 == entry.sha256);
    let only = same.next()?;
    (twins == 1 && same.next().is_none()).then_some(only)
}

pub(super) async fn apply_moves(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    known_ids: &mut KnownIds,
    report: &mut DeleteReport,
) -> DriveResult<()> {
    let created: Vec<LocalChange> = local_changes(cycle.root, state)?
        .into_iter()
        .filter(|change| change.base.is_none())
        .collect();
    let rels: Vec<String> = state.pending_deletes.keys().cloned().collect();
    for rel in rels {
        let Some(entry) = state.entries.get(&rel).cloned() else {
            continue;
        };
        let Some(change) =
            unique_rename(&entry, &state.pending_deletes, &state.entries, &created).cloned()
        else {
            continue;
        };
        let Some(document_id) = known_ids.resolve(cycle, &rel, &entry).await? else {
            continue;
        };
        match cycle
            .client
            .move_document(cycle.bearer, &document_id, &change.file.rel)
            .await
        {
            Ok(()) => {
                let (mtime_unix, size) = metadata(&change.file.abs)?;
                state.entries.remove(&rel);
                state.entries.insert(
                    change.file.rel.clone(),
                    DriveEntryState {
                        document_id: Some(document_id),
                        mtime_unix,
                        size,
                        ..entry
                    },
                );
                state.pending_deletes.remove(&rel);
                report.moved += 1;
            },
            Err(error) if is_refusal(&error) => {
                move_file(&change.file.abs, &cycle.root.file(&rel)?)?;
                state.pending_deletes.remove(&rel);
                notify(state, &rel, format!("the move was refused: {error}"));
                report.restored += 1;
            },
            Err(error) => return Err(error),
        }
        state.save(&cycle.root.state_path())?;
    }
    Ok(())
}
