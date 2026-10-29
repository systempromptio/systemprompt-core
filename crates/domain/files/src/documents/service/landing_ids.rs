//! Node ids for a landing: carried forward from the parent on a blocking
//! thread, with every created node minted a provisional id and flagged as
//! minted, then renumbered inside the landing transaction onto the ids
//! reserved on the author's node id counter. Whether an id is provisional
//! is the flag set where it was minted, never a reading of the id itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::{DocumentId, DocumentNodeId};
use systemprompt_models::documents::Projection;

use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::models::NodeIdEntry;
use crate::documents::nodes::{CarryHints, NodeIdMinter, carry_forward, flatten, mint_all};
use crate::documents::repository::DocumentNodeIdRepository;

const PROVISIONAL: &str = "~";

pub(super) struct CarriedEntry {
    pub(super) entry: NodeIdEntry,
    minted: bool,
}

pub(super) struct Carried {
    pub(super) entries: Vec<CarriedEntry>,
    pub(super) next: Projection,
    pub(super) previous: Option<Projection>,
    pub(super) created: usize,
}

fn mark_minted(entries: Vec<NodeIdEntry>, previous: Option<&Projection>) -> Vec<CarriedEntry> {
    let carried: HashSet<DocumentNodeId> = previous
        .map(|previous| {
            flatten(previous)
                .into_iter()
                .map(|flat| flat.node.id.clone())
                .collect()
        })
        .unwrap_or_default();
    entries
        .into_iter()
        .map(|entry| CarriedEntry {
            minted: !carried.contains(&entry.node_id),
            entry,
        })
        .collect()
}

pub(super) async fn carry(
    next: Projection,
    previous: Option<Projection>,
    hints: CarryHints,
) -> Result<Carried, DocumentsError> {
    blocking(move || {
        let mut minter = NodeIdMinter::new(PROVISIONAL.to_owned(), 0);
        let entries = match previous.as_ref() {
            Some(previous) => carry_forward(previous, &next, &hints, &mut minter),
            None => mint_all(&next, &mut minter),
        };
        let entries = mark_minted(entries, previous.as_ref());
        let created = entries.iter().filter(|carried| carried.minted).count();
        Ok(Carried {
            entries,
            next,
            previous,
            created,
        })
    })
    .await
}

pub(super) async fn reserve(
    tx: &mut sqlx::PgConnection,
    document: &DocumentId,
    writer: &str,
    entries: Vec<CarriedEntry>,
    created: usize,
) -> Result<Vec<NodeIdEntry>, DocumentsError> {
    if created == 0 {
        return Ok(entries.into_iter().map(|carried| carried.entry).collect());
    }
    let count = i64::try_from(created).unwrap_or(i64::MAX);
    let first = DocumentNodeIdRepository::reserve(tx, document, writer, count).await?;
    let mut ordinal = u64::try_from(first - 1).unwrap_or(0);
    Ok(entries
        .into_iter()
        .map(|carried| {
            if !carried.minted {
                return carried.entry;
            }
            ordinal += 1;
            NodeIdEntry {
                node_id: DocumentNodeId::new(format!("{writer}.{ordinal}")),
                ..carried.entry
            }
        })
        .collect())
}
