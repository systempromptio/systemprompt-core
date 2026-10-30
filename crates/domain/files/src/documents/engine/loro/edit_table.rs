//! Table edits by id: a table's or a cell's properties, and inserting or
//! deleting rows and columns. A new cell holds one empty paragraph.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::{Block, BlockKind};
use loro::{LoroDoc, LoroMap, LoroMovableList};
use serde_json::Value;

use super::edit::{EditError, find_cell, find_row, located, rows_of};
use super::rich::{list_of, string_of};
use super::table::{fresh_id, is_table_map, new_entry, to_loro};
use super::write::insert_block;

/// Sets (or with `None` removes) one key of a table's `props`.
pub fn set_table_prop(
    doc: &LoroDoc,
    table_id: &str,
    key: &str,
    value: Option<&Value>,
) -> Result<(), EditError> {
    let table = table(doc, table_id)?;
    let props = table.get_or_create_container("props", LoroMap::new())?;
    set_prop(&props, key, value)
}

/// Sets (or with `None` removes) one key of a cell's `props`.
pub fn set_cell_prop(
    doc: &LoroDoc,
    cell_id: &str,
    key: &str,
    value: Option<&Value>,
) -> Result<(), EditError> {
    let cell = find_cell(doc, cell_id).ok_or_else(|| EditError::NotFound {
        id: cell_id.to_owned(),
    })?;
    let props = cell.get_or_create_container("props", LoroMap::new())?;
    set_prop(&props, key, value)
}

fn set_prop(map: &LoroMap, key: &str, value: Option<&Value>) -> Result<(), EditError> {
    match value {
        Some(value) => map.insert(key, to_loro(value)?)?,
        None if map.get(key).is_some() => map.delete(key)?,
        None => {},
    }
    Ok(())
}

/// A cell's block list, created when the cell has none.
pub fn cell_blocks(doc: &LoroDoc, cell_id: &str) -> Result<LoroMovableList, EditError> {
    let cell = find_cell(doc, cell_id).ok_or_else(|| EditError::NotFound {
        id: cell_id.to_owned(),
    })?;
    Ok(cell.get_or_create_container("blocks", LoroMovableList::new())?)
}

pub(crate) fn table(doc: &LoroDoc, table_id: &str) -> Result<LoroMap, EditError> {
    let map = located(doc, table_id)?.map;
    if is_table_map(&map) {
        Ok(map)
    } else {
        Err(EditError::NotATable {
            id: table_id.to_owned(),
        })
    }
}

/// Inserts an empty row of `columns` cells at `at`; returns the row's id
/// and its cells' ids.
pub fn insert_row(
    doc: &LoroDoc,
    table_id: &str,
    at: usize,
    columns: usize,
) -> Result<(String, Vec<String>), EditError> {
    let table = table(doc, table_id)?;
    let rows = table.get_or_create_container("rows", LoroMovableList::new())?;
    let row = new_entry(&rows, at.min(rows.len()))?;
    let cells = row.insert_container("cells", LoroMovableList::new())?;
    let ids = (0..columns)
        .map(|c| new_cell(&cells, c))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((string_of(&row, "id").unwrap_or_default(), ids))
}

/// Inserts an empty cell at column `at` of every row; returns the new ids.
pub fn insert_column(doc: &LoroDoc, table_id: &str, at: usize) -> Result<Vec<String>, EditError> {
    rows_of(&table(doc, table_id)?)
        .map(|row| {
            let cells = row.get_or_create_container("cells", LoroMovableList::new())?;
            new_cell(&cells, at.min(cells.len()))
        })
        .collect()
}

pub fn delete_row(doc: &LoroDoc, row_id: &str) -> Result<(), EditError> {
    let (_, row) = find_row(doc, row_id).ok_or_else(|| EditError::NotFound {
        id: row_id.to_owned(),
    })?;
    Ok(row.list.delete(row.at, 1)?)
}

/// Deletes column `at` from every row of a table that has it.
pub fn delete_column(doc: &LoroDoc, table_id: &str, at: usize) -> Result<(), EditError> {
    for row in rows_of(&table(doc, table_id)?) {
        if let Some(cells) = list_of(&row, "cells")
            && at < cells.len()
        {
            cells.delete(at, 1)?;
        }
    }
    Ok(())
}

fn new_cell(cells: &LoroMovableList, at: usize) -> Result<String, EditError> {
    let cell = new_entry(cells, at)?;
    cell.insert_container("props", LoroMap::new())?;
    let blocks = cell.insert_container("blocks", LoroMovableList::new())?;
    let paragraph = Block::new(fresh_id(), BlockKind::Paragraph, "");
    insert_block(&blocks, 0, &paragraph)?;
    Ok(string_of(&cell, "id").unwrap_or_default())
}
