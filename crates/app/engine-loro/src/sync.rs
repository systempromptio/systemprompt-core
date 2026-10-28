//! Turning a Loro document into a target `RichText` model with the fewest
//! container operations: blocks are matched by id, so a block that only
//! moved is moved, an edited block has only its changed parts rewritten,
//! and a removed block is deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use loro::LoroDoc;
use systemprompt_files::documents::model::RichText;

use crate::rich::{
    WriteError, block_map, blocks, insert_block, kind_json, string_of, sync_text, text_of,
};

pub fn write_model(doc: &LoroDoc, target: &RichText) -> Result<(), WriteError> {
    let list = blocks(doc);
    let wanted: HashSet<&str> = target
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    let mut ids: Vec<Option<String>> = (0..list.len())
        .map(|at| block_map(&list, at).and_then(|map| string_of(&map, "id")))
        .collect();
    for at in (0..ids.len()).rev() {
        if ids[at].as_deref().is_none_or(|id| !wanted.contains(id)) {
            list.delete(at, 1)?;
            ids.remove(at);
        }
    }
    for (at, block) in target.blocks.iter().enumerate() {
        let found = ids
            .iter()
            .position(|id| id.as_deref() == Some(block.id.as_str()));
        let Some(from) = found else {
            insert_block(&list, at, block)?;
            ids.insert(at, Some(block.id.clone()));
            continue;
        };
        if from != at {
            list.mov(from, at)?;
            let id = ids.remove(from);
            ids.insert(at, id);
        }
        let Some(map) = block_map(&list, at) else {
            continue;
        };
        let kind = kind_json(&block.kind)?;
        if string_of(&map, "kind").as_deref() != Some(kind.as_str()) {
            map.insert("kind", kind)?;
        }
        let text = match text_of(&map) {
            Some(text) => text,
            None => map.insert_container("text", loro::LoroText::new())?,
        };
        sync_text(&text, block)?;
    }
    Ok(())
}
