//! Id-addressed edits on a live Loro document, for callers that translate
//! an edit operation straight into container operations instead of writing
//! a whole model (see [`super::write_model`]).
//!
//! A block is found by id in the root list or in any table cell; a row or
//! cell by its own id. Each edit touches only the containers it names, so
//! it merges with concurrent edits to anything else. Nothing here commits:
//! the caller commits once, under the origin and peer it edits as. Table
//! row, column and property edits are re-exported from `edit_table`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use crate::documents::model::{Block, MarkKind};
use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText, LoroValue, UpdateOptions};

use super::rich::{WriteError, block_map, blocks, list_of, string_of, text_of};
use super::table::is_table_map;

pub use super::edit_table::{
    cell_blocks, delete_column, delete_row, insert_column, insert_row, set_cell_prop,
    set_table_prop,
};
use super::write::{insert_block, sync_attrs, sync_kind};

#[derive(Debug)]
pub enum EditError {
    NotFound { id: String },
    NotATable { id: String },
    Write(WriteError),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { id } => write!(f, "no block, row or cell has id {id}"),
            Self::NotATable { id } => write!(f, "block {id} is not a table"),
            Self::Write(_) => f.write_str("the edit could not be written"),
        }
    }
}

impl std::error::Error for EditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(source) => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<loro::LoroError> for EditError {
    fn from(error: loro::LoroError) -> Self {
        Self::Write(Box::new(error))
    }
}

impl From<WriteError> for EditError {
    fn from(error: WriteError) -> Self {
        Self::Write(error)
    }
}

/// Where a block sits: the list holding it and its index there.
#[derive(Debug, Clone)]
pub struct Located {
    pub list: LoroMovableList,
    pub at: usize,
    pub map: LoroMap,
}

#[must_use]
pub fn find_block(doc: &LoroDoc, id: &str) -> Option<Located> {
    find_in(&blocks(doc), id)
}

fn find_in(list: &LoroMovableList, id: &str) -> Option<Located> {
    for at in 0..list.len() {
        let Some(map) = block_map(list, at) else {
            continue;
        };
        if string_of(&map, "id").as_deref() == Some(id) {
            return Some(Located {
                list: list.clone(),
                at,
                map,
            });
        }
        if let Some(found) = cells_of(&map)
            .find_map(|cell| list_of(&cell, "blocks").and_then(|inner| find_in(&inner, id)))
        {
            return Some(found);
        }
    }
    None
}

pub(crate) fn rows_of(table: &LoroMap) -> impl Iterator<Item = LoroMap> {
    let rows = list_of(table, "rows");
    let len = rows.as_ref().map_or(0, LoroMovableList::len);
    (0..len).filter_map(move |r| rows.as_ref().and_then(|rows| block_map(rows, r)))
}

pub(crate) fn cells_of(table: &LoroMap) -> impl Iterator<Item = LoroMap> {
    rows_of(table).flat_map(|row| {
        let cells = list_of(&row, "cells");
        let len = cells.as_ref().map_or(0, LoroMovableList::len);
        (0..len).filter_map(move |c| cells.as_ref().and_then(|cells| block_map(cells, c)))
    })
}

/// The table, row list index and row map of a row id, searching every table.
#[must_use]
pub fn find_row(doc: &LoroDoc, row_id: &str) -> Option<(LoroMap, Located)> {
    tables(doc).find_map(|table| {
        let rows = list_of(&table, "rows")?;
        (0..rows.len()).find_map(|at| {
            let map = block_map(&rows, at)?;
            (string_of(&map, "id").as_deref() == Some(row_id)).then(|| {
                (
                    table.clone(),
                    Located {
                        list: rows.clone(),
                        at,
                        map,
                    },
                )
            })
        })
    })
}

#[must_use]
pub fn find_cell(doc: &LoroDoc, cell_id: &str) -> Option<LoroMap> {
    tables(doc).find_map(|table| {
        cells_of(&table).find(|cell| string_of(cell, "id").as_deref() == Some(cell_id))
    })
}

fn tables(doc: &LoroDoc) -> impl Iterator<Item = LoroMap> {
    let list = blocks(doc);
    (0..list.len()).filter_map(move |at| block_map(&list, at).filter(is_table_map))
}

pub(crate) fn located(doc: &LoroDoc, id: &str) -> Result<Located, EditError> {
    find_block(doc, id).ok_or_else(|| EditError::NotFound { id: id.to_owned() })
}

fn text(map: &LoroMap) -> Result<LoroText, EditError> {
    match text_of(map) {
        Some(text) => Ok(text),
        None => Ok(map.insert_container("text", LoroText::new())?),
    }
}

/// Replaces a block's text with the fewest text operations.
pub fn set_text(doc: &LoroDoc, block_id: &str, value: &str) -> Result<(), EditError> {
    let text = text(&located(doc, block_id)?.map)?;
    text.update(value, UpdateOptions::default())
        .map_err(|error| EditError::Write(Box::new(error)))
}

/// Inserts at a Unicode scalar offset of a block's text.
pub fn insert_text(doc: &LoroDoc, block_id: &str, at: usize, value: &str) -> Result<(), EditError> {
    Ok(text(&located(doc, block_id)?.map)?.insert(at, value)?)
}

pub fn delete_text(doc: &LoroDoc, block_id: &str, at: usize, len: usize) -> Result<(), EditError> {
    Ok(text(&located(doc, block_id)?.map)?.delete(at, len)?)
}

/// Marks `range` of a block's text with `kind`, stored as
/// [`super::rich::TEXT_STYLES`] expects: `true` for a flag mark, the value's
/// string otherwise.
pub fn mark(
    doc: &LoroDoc,
    block_id: &str,
    range: std::ops::Range<usize>,
    kind: &MarkKind,
) -> Result<(), EditError> {
    let value: LoroValue = match kind.value() {
        "" => true.into(),
        value => value.into(),
    };
    Ok(text(&located(doc, block_id)?.map)?.mark(range, kind.key(), value)?)
}

/// Removes the mark stored under `key` (a [`MarkKind::key`]) from `range`.
pub fn unmark(
    doc: &LoroDoc,
    block_id: &str,
    range: std::ops::Range<usize>,
    key: &str,
) -> Result<(), EditError> {
    Ok(text(&located(doc, block_id)?.map)?.unmark(range, key)?)
}

/// Sets a block's kind and attributes from `block`, leaving its text alone.
pub fn set_kind(doc: &LoroDoc, block: &Block) -> Result<(), EditError> {
    let map = located(doc, &block.id)?.map;
    sync_kind(&map, block)?;
    sync_attrs(&map, &block.attrs)?;
    Ok(())
}

/// Inserts `block` next to `anchor` (after it, or before when `before`).
pub fn insert_block_near(
    doc: &LoroDoc,
    anchor: &str,
    before: bool,
    block: &Block,
) -> Result<(), EditError> {
    let Located { list, at, .. } = located(doc, anchor)?;
    insert_block(&list, if before { at } else { at + 1 }, block)?;
    Ok(())
}

/// Inserts `block` at `at` of the root list, clamped to its length.
pub fn insert_root_block(doc: &LoroDoc, at: usize, block: &Block) -> Result<(), EditError> {
    let list = blocks(doc);
    insert_block(&list, at.min(list.len()), block)?;
    Ok(())
}

pub fn delete_block(doc: &LoroDoc, block_id: &str) -> Result<(), EditError> {
    let Located { list, at, .. } = located(doc, block_id)?;
    Ok(list.delete(at, 1)?)
}

/// Moves a block to `to` within the list that holds it.
pub fn move_block(doc: &LoroDoc, block_id: &str, to: usize) -> Result<(), EditError> {
    let Located { list, at, .. } = located(doc, block_id)?;
    let to = to.min(list.len().saturating_sub(1));
    if to != at {
        list.mov(at, to)?;
    }
    Ok(())
}
