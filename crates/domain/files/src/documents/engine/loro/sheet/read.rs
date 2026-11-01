//! Reads a document's [`SHEETS_CONTAINER`] into a [`SheetModel`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{Container, LoroDoc, LoroMap, LoroValue, ValueOrContainer};

use super::SHEETS_CONTAINER;
use super::model::{Sheet, SheetCell, SheetModel};

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum SheetReadError {
    #[error("sheet list entry {at} is not a sheet")]
    NotASheet { at: usize },
    #[error("sheet {at} has no id")]
    MissingId { at: usize },
}

pub fn read(doc: &LoroDoc) -> Result<SheetModel, SheetReadError> {
    let list = doc.get_movable_list(SHEETS_CONTAINER);
    let sheets = (0..list.len())
        .map(|at| match list.get(at) {
            Some(ValueOrContainer::Container(Container::Map(map))) => read_sheet(at, &map),
            _ => Err(SheetReadError::NotASheet { at }),
        })
        .collect::<Result<_, _>>()?;
    Ok(SheetModel { sheets })
}

pub(super) fn read_sheet(at: usize, map: &LoroMap) -> Result<Sheet, SheetReadError> {
    let id = string(map, "id").ok_or(SheetReadError::MissingId { at })?;
    let mut sheet = Sheet {
        id,
        name: string(map, "name").unwrap_or_default(),
        rows: id_list(map, "rows"),
        cols: id_list(map, "cols"),
        ..Sheet::default()
    };
    if let Some(ValueOrContainer::Container(Container::Map(cells))) = map.get("cells") {
        cells.for_each(|key, value| {
            if let ValueOrContainer::Container(Container::Map(cell)) = value {
                sheet.cells.insert(key.to_owned(), read_cell(&cell));
            }
        });
    }
    if let Some(ValueOrContainer::Container(Container::List(merges))) = map.get("merges") {
        sheet.merges = strings(merges.to_vec());
    }
    Ok(sheet)
}

fn read_cell(map: &LoroMap) -> SheetCell {
    SheetCell {
        v: string(map, "v").unwrap_or_default(),
        fmt: string(map, "fmt"),
        style: string(map, "style"),
    }
}

pub(super) fn string(map: &LoroMap, key: &str) -> Option<String> {
    match map.get(key)? {
        ValueOrContainer::Value(LoroValue::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn id_list(map: &LoroMap, key: &str) -> Vec<String> {
    match map.get(key) {
        Some(ValueOrContainer::Container(Container::MovableList(list))) => {
            let mut seen = std::collections::HashSet::new();
            strings(list.to_vec())
                .into_iter()
                .filter(|id| seen.insert(id.clone()))
                .collect()
        },
        _ => Vec::new(),
    }
}

fn strings(values: Vec<LoroValue>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| match value {
            LoroValue::String(text) => Some(text.to_string()),
            _ => None,
        })
        .collect()
}
