//! Cell-sized edits to a document's sheets, the writes the agent ops path
//! and the checkpoint upgrade commit.
//!
//! Every function touches only the containers its edit names: setting a
//! cell writes that cell's map, inserting a row inserts one id. Growing a
//! sheet to reach an address appends fresh ids at the end of `rows` or
//! `cols`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{Container, LoroDoc, LoroMap, LoroMovableList, LoroValue, ValueOrContainer};

use super::SHEETS_CONTAINER;
use super::model::{cell_key, new_id};
use super::read::string;
use crate::documents::engine::loro::rich::WriteError;

#[must_use]
pub fn sheet_map(doc: &LoroDoc, name: &str) -> Option<LoroMap> {
    let list = doc.get_movable_list(SHEETS_CONTAINER);
    (0..list.len()).find_map(|at| match list.get(at) {
        Some(ValueOrContainer::Container(Container::Map(map)))
            if string(&map, "name").as_deref() == Some(name) =>
        {
            Some(map)
        },
        _ => None,
    })
}

fn axis(sheet: &LoroMap, key: &str) -> Result<LoroMovableList, WriteError> {
    Ok(sheet.get_or_create_container(key, LoroMovableList::new())?)
}

fn id_at(list: &LoroMovableList, index: usize) -> Option<String> {
    match list.get(index)? {
        ValueOrContainer::Value(LoroValue::String(id)) => Some(id.to_string()),
        _ => None,
    }
}

fn ensure_axis(sheet: &LoroMap, key: &str, index: usize) -> Result<String, WriteError> {
    let list = axis(sheet, key)?;
    while list.len() <= index {
        list.push(new_id())?;
    }
    id_at(&list, index).ok_or_else(|| format!("{key}[{index}] is not an id").into())
}

pub fn ensure_cell_key(sheet: &LoroMap, col: u32, row: u32) -> Result<String, WriteError> {
    let row_id = ensure_axis(sheet, "rows", index_of(row)?)?;
    let col_id = ensure_axis(sheet, "cols", index_of(col)?)?;
    Ok(cell_key(&row_id, &col_id))
}

fn index_of(one_based: u32) -> Result<usize, WriteError> {
    usize::try_from(one_based)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .ok_or_else(|| "addresses are 1-based".into())
}

pub fn set_cell_input(sheet: &LoroMap, key: &str, v: &str) -> Result<(), WriteError> {
    let cells = sheet.get_or_create_container("cells", LoroMap::new())?;
    if v.is_empty() {
        cells.delete(key)?;
        return Ok(());
    }
    let cell = cells.get_or_create_container(key, LoroMap::new())?;
    cell.insert("v", v)?;
    Ok(())
}

pub fn insert_row_after(sheet: &LoroMap, row: u32) -> Result<String, WriteError> {
    let rows = axis(sheet, "rows")?;
    let at = usize::try_from(row)?.min(rows.len());
    let id = new_id();
    rows.insert(at, id.as_str())?;
    Ok(id)
}

#[must_use]
pub fn row_number(sheet: &LoroMap, row_id: &str) -> Option<u32> {
    let rows = axis(sheet, "rows").ok()?;
    (0..rows.len())
        .find(|at| id_at(&rows, *at).as_deref() == Some(row_id))
        .and_then(|at| u32::try_from(at + 1).ok())
}
