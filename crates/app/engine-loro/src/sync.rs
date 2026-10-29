//! Turning a Loro document into a target `RichText` model with the fewest
//! container operations: blocks are matched by id, so a block that only
//! moved is moved, an edited block has only its changed parts rewritten,
//! and a removed block is deleted.
//!
//! The target is written left to right. Blocks still waiting to be placed
//! keep their original index, and a Fenwick tree over those indices gives
//! each one's current list position, so a write of `n` blocks costs
//! `O(n log n)` however the blocks moved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use loro::{LoroDoc, LoroMovableList};
use systemprompt_files::documents::model::{Block, RichText};

use crate::rich::{
    WriteError, block_map, blocks, insert_block, kind_json, string_of, sync_attrs, sync_settings,
    sync_text, text_of,
};

struct Waiting {
    tree: Vec<usize>,
}

impl Waiting {
    fn all(len: usize) -> Self {
        let mut tree = vec![0; len + 1];
        for at in 1..=len {
            tree[at] += 1;
            let parent = at + (at & at.wrapping_neg());
            if parent <= len {
                tree[parent] += tree[at];
            }
        }
        Self { tree }
    }

    fn before(&self, original: usize) -> usize {
        let mut sum = 0;
        let mut at = original;
        while at > 0 {
            sum += self.tree[at];
            at &= at - 1;
        }
        sum
    }

    fn place(&mut self, original: usize) {
        let mut at = original + 1;
        while at < self.tree.len() {
            self.tree[at] -= 1;
            at += at & at.wrapping_neg();
        }
    }
}

pub fn write_model(doc: &LoroDoc, target: &RichText) -> Result<(), WriteError> {
    sync_settings(doc, target.document.as_deref())?;
    let list = blocks(doc);
    let wanted: HashSet<&str> = target
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    let held: Vec<Option<String>> = (0..list.len())
        .map(|at| block_map(&list, at).and_then(|map| string_of(&map, "id")))
        .collect();
    let mut original: HashMap<&str, usize> = HashMap::with_capacity(wanted.len());
    let mut keep = vec![false; held.len()];
    for (at, id) in held.iter().enumerate() {
        if let Some(id) = id.as_deref()
            && wanted.contains(id)
            && !original.contains_key(id)
        {
            original.insert(id, original.len());
            keep[at] = true;
        }
    }
    for at in (0..held.len()).rev() {
        if !keep[at] {
            list.delete(at, 1)?;
        }
    }
    let mut waiting = Waiting::all(original.len());
    let mut placed: HashSet<&str> = HashSet::with_capacity(target.blocks.len());
    for (at, block) in target.blocks.iter().enumerate() {
        let id = block.id.as_str();
        let first = placed.insert(id);
        let Some(&from_original) = original.get(id).filter(|_| first) else {
            insert_block(&list, at, block)?;
            continue;
        };
        let from = at + waiting.before(from_original);
        waiting.place(from_original);
        if from != at {
            list.mov(from, at)?;
        }
        sync_block(&list, at, block)?;
    }
    Ok(())
}

fn sync_block(list: &LoroMovableList, at: usize, block: &Block) -> Result<(), WriteError> {
    let Some(map) = block_map(list, at) else {
        return Ok(());
    };
    let kind = kind_json(&block.kind)?;
    if string_of(&map, "kind").as_deref() != Some(kind.as_str()) {
        map.insert("kind", kind)?;
    }
    sync_attrs(&map, &block.attrs)?;
    let text = match text_of(&map) {
        Some(text) => text,
        None => map.insert_container("text", loro::LoroText::new())?,
    };
    sync_text(&text, block)
}
