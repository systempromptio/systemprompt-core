//! Writing one block into its Loro map: a new block's containers, and the
//! attribute, kind and text syncs an edited block needs.
//!
//! A table block is written in the container shape of [`super::table`]; any
//! other block keeps its type and attributes as JSON under `kind`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind, Mark, MarkKind};
use loro::{LoroError, LoroMap, LoroMovableList, LoroText, LoroValue};

use super::rich::{WriteError, read_text, string_of};
use super::table::{TABLE_KIND, drop_table, write_table};

pub fn insert_block(list: &LoroMovableList, at: usize, block: &Block) -> Result<(), WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", block.id.as_str())?;
    match &block.kind {
        BlockKind::Table { rows, props } => {
            map.insert("kind", TABLE_KIND)?;
            write_table(&map, rows, props)?;
        },
        kind => map.insert("kind", kind_json(kind)?)?,
    }
    sync_attrs(&map, &block.attrs)?;
    let text = map.insert_container("text", LoroText::new())?;
    text.insert(0, &block.text)?;
    mark_all(&text, &block.marks)?;
    Ok(())
}

pub(crate) fn sync_kind(map: &LoroMap, block: &Block) -> Result<(), WriteError> {
    if let BlockKind::Table { rows, props } = &block.kind {
        return write_table(map, rows, props);
    }
    drop_table(map)?;
    let kind = kind_json(&block.kind)?;
    if string_of(map, "kind").as_deref() != Some(kind.as_str()) {
        map.insert("kind", kind)?;
    }
    Ok(())
}

pub(crate) fn sync_attrs(map: &LoroMap, attrs: &ParaAttrs) -> Result<(), WriteError> {
    let held = string_of(map, "attrs");
    if attrs.is_default() {
        if held.is_some() {
            map.delete("attrs")?;
        }
        return Ok(());
    }
    let json = serde_json::to_string(attrs)?;
    if held.as_deref() != Some(json.as_str()) {
        map.insert("attrs", json)?;
    }
    Ok(())
}

pub(crate) fn kind_json(kind: &BlockKind) -> serde_json::Result<String> {
    serde_json::to_string(kind)
}

pub(crate) fn mark_all(text: &LoroText, marks: &[Mark]) -> Result<(), LoroError> {
    for mark in marks {
        let value: LoroValue = match mark.kind.value() {
            "" => true.into(),
            value => value.into(),
        };
        text.mark(mark.start..mark.end, mark.kind.key(), value)?;
    }
    Ok(())
}

pub fn sync_text(text: &LoroText, block: &Block) -> Result<(), WriteError> {
    let (held, mut marks) = read_text(text);
    if held != block.text {
        text.update(&block.text, loro::UpdateOptions::default())?;
        marks = read_text(text).1;
    }
    if marks != block.marks {
        let len = text.len_unicode();
        if len > 0 {
            for key in MarkKind::KEYS {
                text.unmark(0..len, key)?;
            }
        }
        mark_all(text, &block.marks)?;
    }
    Ok(())
}
