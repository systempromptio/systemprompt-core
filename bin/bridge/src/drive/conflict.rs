//! A commit the gateway refused because its base was no longer the head.
//!
//! The local edit is renamed to `<stem> (conflict from <host>).<ext>`, the
//! head revision is fetched into the original name through a verified write,
//! and a [`ConflictNotice`] is kept in the state until the sibling is gone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId};

use super::Cycle;
use super::error::{DriveError, DriveResult};
use super::state::{ConflictNotice, DriveState};
use super::write::{free_conflict_sibling, move_file, synced_entry, write_verified};

/// The revision that takes the original name when a local edit is kept
/// aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadRef {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub sha256: ContentHash,
}

pub async fn resolve(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    head_revision_id: &DocumentRevisionId,
) -> DriveResult<ConflictNotice> {
    let manifest = cycle.client.manifest(cycle.bearer).await?;
    let head = manifest
        .entries
        .into_iter()
        .find(|entry| entry.path == rel)
        .ok_or_else(|| DriveError::HeadMissing {
            path: rel.to_owned(),
        })?;
    if &head.head_revision_id != head_revision_id {
        tracing::info!(path = %rel, "drive head moved again since the conflict; using the newer head");
    }
    let head = HeadRef {
        document_id: head.document_id,
        revision_id: head.head_revision_id,
        sha256: head.content_sha256,
    };
    let bytes = cycle
        .client
        .download_blob(cycle.bearer, &head.sha256)
        .await?;
    keep_local_beside(cycle, state, rel, &bytes, head)
}

pub fn keep_local_beside(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    rel: &str,
    bytes: &[u8],
    head: HeadRef,
) -> DriveResult<ConflictNotice> {
    let original = cycle.root.file(rel)?;
    let sibling = free_conflict_sibling(cycle.root, rel, cycle.host_name)?;
    let sibling_path = cycle.root.file(&sibling)?;
    let moved = original.exists();
    if moved {
        move_file(&original, &sibling_path)?;
    }
    if let Err(error) = write_verified(&original, bytes, &head.sha256) {
        if moved && let Err(restore) = move_file(&sibling_path, &original) {
            tracing::error!(%restore, path = %rel, "drive could not restore a local edit after a failed head write");
        }
        return Err(error);
    }
    let notice = ConflictNotice {
        path: rel.to_owned(),
        sibling,
        head_revision_id: head.revision_id.clone(),
        detected_at: Utc::now(),
    };
    let entry = synced_entry(
        &original,
        Some(head.document_id),
        head.revision_id,
        head.sha256,
    )?;
    state.entries.insert(rel.to_owned(), entry);
    state.conflicts.push(notice.clone());
    state.save(&cycle.root.state_path())?;
    tracing::warn!(
        path = %notice.path,
        sibling = %notice.sibling,
        head_revision_id = %notice.head_revision_id,
        "drive kept a conflicting local edit beside the head revision"
    );
    Ok(notice)
}

pub fn prune_resolved(root: &super::DriveRoot, state: &mut DriveState) -> bool {
    let before = state.conflicts.len();
    state
        .conflicts
        .retain(|notice| root.file(&notice.sibling).is_ok_and(|p| p.exists()));
    before != state.conflicts.len()
}
