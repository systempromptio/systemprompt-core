//! Writes a [`SheetModel`] into a document's [`SHEETS_CONTAINER`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroList, LoroMap, LoroMovableList};

use super::SHEETS_CONTAINER;
use super::model::{Sheet, SheetCell, SheetModel};
use crate::documents::engine::loro::rich::WriteError;

pub fn write_sheets(doc: &LoroDoc, model: &SheetModel) -> Result<(), WriteError> {
    let list = doc.get_movable_list(SHEETS_CONTAINER);
    list.clear()?;
    for sheet in &model.sheets {
        let map = list.push_container(LoroMap::new())?;
        write_sheet(&map, sheet)?;
    }
    Ok(())
}

fn write_sheet(map: &LoroMap, sheet: &Sheet) -> Result<(), WriteError> {
    map.insert("id", sheet.id.as_str())?;
    map.insert("name", sheet.name.as_str())?;
    let rows = map.insert_container("rows", LoroMovableList::new())?;
    for id in &sheet.rows {
        rows.push(id.as_str())?;
    }
    let cols = map.insert_container("cols", LoroMovableList::new())?;
    for id in &sheet.cols {
        cols.push(id.as_str())?;
    }
    let cells = map.insert_container("cells", LoroMap::new())?;
    for (key, cell) in &sheet.cells {
        let target = cells.insert_container(key, LoroMap::new())?;
        write_cell(&target, cell)?;
    }
    let merges = map.insert_container("merges", LoroList::new())?;
    for merge in &sheet.merges {
        merges.push(merge.as_str())?;
    }
    Ok(())
}

pub(super) fn write_cell(map: &LoroMap, cell: &SheetCell) -> Result<(), WriteError> {
    map.insert("v", cell.v.as_str())?;
    match &cell.fmt {
        Some(fmt) => map.insert("fmt", fmt.as_str())?,
        None => map.delete("fmt")?,
    }
    match &cell.style {
        Some(style) => map.insert("style", style.as_str())?,
        None => map.delete("style")?,
    }
    Ok(())
}

pub fn seed_sheets(doc: &LoroDoc, model: &SheetModel) -> Result<bool, WriteError> {
    let current = super::read(doc)?;
    let placeholder = matches!(current.sheets.as_slice(), [only] if only.cells.is_empty());
    if !current.sheets.is_empty() && !placeholder {
        return Ok(false);
    }
    write_sheets(doc, model)?;
    Ok(true)
}
