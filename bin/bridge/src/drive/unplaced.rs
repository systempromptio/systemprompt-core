//! Bringing a document that could not be placed onto the device.
//!
//! A document refused by [`super::names`] or by a collision is kept as an
//! unplaced conflict. The change that named it has passed, so nothing in the
//! feed brings it back: before each feed the documents still unplaced are
//! looked up in the manifest and placed at the path the gateway now gives
//! them, through the same rename-and-place step a change takes. A rename
//! with no revision, for a document this device has not placed, is resolved
//! the same way. A document absent from the manifest keeps its entry until
//! its deletion arrives. A failed manifest fetch fails the pull, so the
//! cursor stays on the change and the next cycle repeats it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::DriveManifest;

use super::Cycle;
use super::conflict::HeadRef;
use super::error::DriveResult;
use super::pull::{PullReport, move_and_place, skip_if_unplaceable};
use super::state::DriveState;

pub async fn place_from_manifest(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    document_id: &DocumentId,
    report: &mut PullReport,
) -> DriveResult<()> {
    let manifest = cycle.client.manifest(cycle.bearer).await?;
    place_listed(cycle, state, &manifest, document_id, report).await
}

pub async fn retry(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    report: &mut PullReport,
) -> DriveResult<()> {
    let pending: Vec<DocumentId> = state
        .conflicts
        .iter()
        .filter(|notice| notice.unplaced)
        .filter_map(|notice| notice.document_id.clone())
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    let manifest = cycle.client.manifest(cycle.bearer).await?;
    for document_id in &pending {
        place_listed(cycle, state, &manifest, document_id, report).await?;
    }
    state.save(&cycle.root.state_path())?;
    Ok(())
}

async fn place_listed(
    cycle: Cycle<'_>,
    state: &mut DriveState,
    manifest: &DriveManifest,
    document_id: &DocumentId,
    report: &mut PullReport,
) -> DriveResult<()> {
    let Some(entry) = manifest
        .entries
        .iter()
        .find(|entry| &entry.document_id == document_id)
    else {
        tracing::debug!(%document_id, "drive found no head for an unplaced document; its deletion will clear it");
        return Ok(());
    };
    let head = HeadRef {
        document_id: entry.document_id.clone(),
        revision_id: entry.head_revision_id.clone(),
        sha256: entry.content_sha256.clone(),
    };
    let placed = move_and_place(cycle, state, &entry.path, head, report).await;
    skip_if_unplaceable(
        state,
        placed,
        document_id,
        Some(&entry.head_revision_id),
        report,
    )
}
