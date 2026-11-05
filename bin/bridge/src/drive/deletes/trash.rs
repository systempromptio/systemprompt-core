//! Sending a confirmed delete to the trash, and bringing back a file the
//! server refused or that the user chose to restore.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::{DriveManifest, DriveTrashResponse};

use super::moves::KnownIds;
use super::{DeleteReport, is_gone, is_refusal, notify};
use crate::drive::Cycle;
use crate::drive::error::DriveResult;
use crate::drive::state::DriveState;
use crate::drive::write::{synced_entry, write_verified};

pub(super) async fn trash_one(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    known_ids: &mut KnownIds,
    report: &mut DeleteReport,
) -> DriveResult<()> {
    let Some(entry) = state.entries.get(rel).cloned() else {
        state.pending_deletes.remove(rel);
        return Ok(());
    };
    let Some(document_id) = known_ids.resolve(cycle, rel, &entry).await? else {
        tracing::debug!(path = %rel, "drive has no document id for an absent file yet");
        return Ok(());
    };
    let outcome = cycle
        .client
        .trash(cycle.bearer, &document_id, &entry.revision_id)
        .await;
    match outcome {
        Ok(DriveTrashResponse::Trashed) => {
            state.entries.remove(rel);
            state.pending_deletes.remove(rel);
            report.trashed += 1;
            tracing::info!(path = %rel, "drive moved a locally deleted document to the trash");
        },
        Ok(DriveTrashResponse::Conflict { .. }) => {
            restore_one(cycle, state, rel, &document_id, report).await?;
            notify(
                state,
                rel,
                "it changed on the server, so it was not deleted".to_owned(),
            );
        },
        Err(error) if is_gone(&error) => {
            state.entries.remove(rel);
            state.pending_deletes.remove(rel);
        },
        Err(error) if is_refusal(&error) => {
            restore_one(cycle, state, rel, &document_id, report).await?;
            notify(state, rel, format!("the delete was refused: {error}"));
        },
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(super) async fn restore_all(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    report: &mut DeleteReport,
) -> DriveResult<()> {
    let manifest = cycle.client.manifest(cycle.bearer).await?;
    let by_path: BTreeMap<&str, &DocumentId> = manifest
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), &entry.document_id))
        .collect();
    let rels: Vec<String> = state.pending_deletes.keys().cloned().collect();
    for rel in rels {
        let Some(entry) = state.entries.get(&rel) else {
            continue;
        };
        let known = entry.document_id.as_ref();
        let Some(id) = known
            .or_else(|| by_path.get(rel.as_str()).copied())
            .cloned()
        else {
            continue;
        };
        if restore_from(cycle, state, &rel, &id, &manifest).await? {
            report.restored += 1;
        }
    }
    Ok(())
}

async fn restore_one(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    document_id: &DocumentId,
    report: &mut DeleteReport,
) -> DriveResult<()> {
    let manifest = cycle.client.manifest(cycle.bearer).await?;
    if restore_from(cycle, state, rel, document_id, &manifest).await? {
        report.restored += 1;
    }
    Ok(())
}

async fn restore_from(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    document_id: &DocumentId,
    manifest: &DriveManifest,
) -> DriveResult<bool> {
    state.pending_deletes.remove(rel);
    let Some(head) = manifest
        .entries
        .iter()
        .find(|entry| &entry.document_id == document_id)
        .cloned()
    else {
        state.entries.remove(rel);
        return Ok(false);
    };
    let bytes = cycle
        .client
        .download_blob(cycle.bearer, &head.content_sha256)
        .await?;
    let path = cycle.root.file(&head.path)?;
    write_verified(&path, &bytes, &head.content_sha256)?;
    let entry = synced_entry(
        &path,
        Some(head.document_id),
        head.head_revision_id,
        head.content_sha256,
    )?;
    state.entries.remove(rel);
    state.entries.insert(head.path, entry);
    Ok(true)
}
