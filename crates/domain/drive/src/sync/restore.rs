//! Putting a locally deleted document back from the instance's head, when the
//! user restores held deletes or the instance refuses or outdates a trash.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sp_protocol::ids::DocumentId;
use sp_protocol::wire::drive::FeedEntry;

use super::cycle::{Cycle, Report};
use super::disk::{synced_entry, write_verified};
use super::error::SyncError;
use super::index::SyncIndex;
use super::status::ActivityKind;

pub(super) async fn restore_all(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    report: &mut Report,
) -> Result<(), SyncError> {
    let manifest = cx.feed.manifest(cx.token).await?;
    let by_path: BTreeMap<String, DocumentId> = manifest
        .entries
        .iter()
        .map(|entry| (entry.path.clone(), entry.document_id.clone()))
        .collect();
    let rels: Vec<String> = index.pending_deletes.keys().cloned().collect();
    for rel in rels {
        let Some(entry) = index.entries.get(&rel) else {
            index.pending_deletes.remove(&rel);
            continue;
        };
        let Some(id) = entry
            .document_id
            .clone()
            .or_else(|| by_path.get(&rel).cloned())
        else {
            index.pending_deletes.remove(&rel);
            continue;
        };
        let head = manifest
            .entries
            .iter()
            .find(|listed| listed.document_id == id)
            .cloned();
        restore_from(cx, index, &rel, head, report).await?;
    }
    Ok(())
}

pub(super) async fn restore_one(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    document_id: &DocumentId,
    report: &mut Report,
) -> Result<(), SyncError> {
    let manifest = cx.feed.manifest(cx.token).await?;
    let head = manifest
        .entries
        .into_iter()
        .find(|entry| &entry.document_id == document_id);
    restore_from(cx, index, rel, head, report).await
}

async fn restore_from(
    cx: Cycle<'_>,
    index: &mut SyncIndex,
    rel: &str,
    head: Option<FeedEntry>,
    report: &mut Report,
) -> Result<(), SyncError> {
    index.pending_deletes.remove(rel);
    let Some(head) = head else {
        index.entries.remove(rel);
        return Ok(());
    };
    let bytes = cx.feed.blob(cx.token, &head.content_sha256).await?;
    let path = cx.root.file(&head.path)?;
    write_verified(cx.root, &path, &bytes, &head.content_sha256)?;
    let entry = synced_entry(
        &path,
        Some(head.document_id),
        head.head_revision_id,
        head.content_sha256,
    )?;
    index.entries.remove(rel);
    index.entries.insert(head.path.clone(), entry);
    report.note(&head.path, ActivityKind::Restored);
    Ok(())
}
