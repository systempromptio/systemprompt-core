//! The part of a rich document one import changed, read from the Loro diff
//! between the frontiers before and after it, so the update is validated in
//! proportion to what it touched rather than to the whole document.
//!
//! Every changed container is walked up to the top-level block that holds
//! it; inserted root entries count as touched too, and the settings map
//! touches the document settings. The touched blocks are read whole (a
//! table with all its cells). The scope is refused, and the caller falls
//! back to the full model, when the diff touches a root container other
//! than [`BLOCKS_CONTAINER`] and [`DOCUMENT_CONTAINER`], when a container's
//! path cannot be resolved, when more than [`MAX_TOUCHED`] blocks changed,
//! when the root list outgrows the block limit, or when a touched block
//! holds a `Raw` block (whose source must be compared with the model before
//! the update).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use crate::documents::model::{Block, BlockKind, Limits, RichText};
use loro::event::{Diff, ListDiffItem};
use loro::{ContainerID, ContainerTrait, Frontiers, Index, LoroDoc};

use super::rich::{
    BLOCKS_CONTAINER, DOCUMENT_CONTAINER, block_map, blocks, read_block, read_settings,
};

pub const MAX_TOUCHED: usize = 256;

#[must_use]
pub fn touched(doc: &LoroDoc, before: &Frontiers, after: &Frontiers) -> Option<RichText> {
    let list = blocks(doc);
    if list.len() > Limits::default().blocks {
        return None;
    }
    let root = list.id();
    let diff = doc.diff(before, after).ok()?;
    let mut at = BTreeSet::new();
    let mut settings = false;
    for (id, change) in diff.iter() {
        if let ContainerID::Root { name, .. } = id {
            match name.as_str() {
                BLOCKS_CONTAINER => inserted(change, &mut at),
                DOCUMENT_CONTAINER => settings = true,
                _ => return None,
            }
            continue;
        }
        let path = doc.get_path_to_container(id)?;
        match path.get(1) {
            Some((_, Index::Seq(index))) if path.first().is_some_and(|(top, _)| *top == root) => {
                at.insert(*index);
            },
            _ => return None,
        }
        if at.len() > MAX_TOUCHED {
            return None;
        }
    }
    let blocks = at
        .into_iter()
        .filter(|index| *index < list.len())
        .map(|index| block_map(&list, index).and_then(|map| read_block(index, &map).ok()))
        .collect::<Option<Vec<_>>>()?;
    if blocks.iter().any(holds_raw) {
        return None;
    }
    Some(RichText {
        blocks,
        document: if settings { read_settings(doc) } else { None },
    })
}

fn inserted(change: &Diff<'_>, into: &mut BTreeSet<usize>) {
    let Diff::List(items) = change else {
        return;
    };
    let mut position = 0;
    for item in items {
        match item {
            ListDiffItem::Retain { retain } => position += retain,
            ListDiffItem::Insert { insert, .. } => {
                into.extend(position..position + insert.len());
                position += insert.len();
            },
            ListDiffItem::Delete { .. } => {},
        }
    }
}

fn holds_raw(block: &Block) -> bool {
    matches!(block.kind, BlockKind::Raw { .. })
        || block
            .kind
            .nested()
            .iter()
            .any(|part| part.blocks.iter().any(holds_raw))
}
