//! A table block held as Loro containers, so concurrent edits to different
//! cells, rows or properties merge instead of replacing the table.
//!
//! The block map keeps `kind` = [`TABLE_KIND`] and adds `props`, a map of
//! the table's own properties (`columns`, `header_rows`, `style`), and `rows`,
//! a movable list of row maps `{id, cells}`. `cells` is a movable list of cell
//! maps `{id, props, blocks}`: `props` holds the cell's `shade`, `align`,
//! `valign`, `colspan` and `rowspan` (absent when default; a span of 1 is never
//! written), and every props value is its JSON text, raw numbers and flags also
//! read, and `blocks` is a movable list of block maps in the same shape as the
//! root list. A browser editor mirrors this shape exactly.
//!
//! The model's `TableProps::cells` is sparse, so a table reads back in
//! canonical form ([`canonical_table`]): cell styling beyond the table's
//! rows and cells, trailing default cells and trailing empty rows are
//! dropped, a span of 1 reads as the default 0, and a cell carries no
//! document settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::layout::{CellProps, TableProps};
use crate::documents::model::{BlockKind, RichText};
use loro::{LoroMap, LoroMovableList, LoroValue, ValueOrContainer};
use serde_json::{Map, Value};

use super::rich::{ReadError, WriteError, block_map, list_of, map_of, read_blocks, string_of};
use super::sync::write_blocks;

pub const TABLE_KIND: &str = "{\"type\":\"table\"}";

#[must_use]
pub(crate) fn is_table_map(map: &LoroMap) -> bool {
    list_of(map, "rows").is_some()
}

#[must_use]
pub fn fresh_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn read_table(id: &str, map: &LoroMap) -> Result<BlockKind, ReadError> {
    let bad = || ReadError::Table { id: id.to_owned() };
    let mut props: TableProps = map_of(map, "props")
        .map(|props| from_props(&props))
        .transpose()
        .map_err(|_| bad())?
        .unwrap_or_default();
    let list = list_of(map, "rows").ok_or_else(bad)?;
    let mut rows = Vec::with_capacity(list.len());
    let mut styles = Vec::with_capacity(list.len());
    for r in 0..list.len() {
        let row = block_map(&list, r).ok_or_else(bad)?;
        let cells = list_of(&row, "cells").ok_or_else(bad)?;
        let mut contents = Vec::with_capacity(cells.len());
        let mut row_styles = Vec::with_capacity(cells.len());
        for c in 0..cells.len() {
            let cell = block_map(&cells, c).ok_or_else(bad)?;
            let blocks = match list_of(&cell, "blocks") {
                Some(blocks) => read_blocks(&blocks)?,
                None => Vec::new(),
            };
            contents.push(RichText {
                blocks,
                document: None,
            });
            let style: CellProps = map_of(&cell, "props")
                .map(|props| from_props(&props))
                .transpose()
                .map_err(|_| bad())?
                .unwrap_or_default();
            row_styles.push(style);
        }
        rows.push(contents);
        styles.push(row_styles);
    }
    props.cells = styles;
    canonical_table(&mut rows, &mut props);
    Ok(BlockKind::Table { rows, props })
}

pub(crate) fn write_table(
    map: &LoroMap,
    rows: &[Vec<RichText>],
    props: &TableProps,
) -> Result<(), WriteError> {
    if string_of(map, "kind").as_deref() != Some(TABLE_KIND) {
        map.insert("kind", TABLE_KIND)?;
    }
    let mut own = props.clone();
    own.cells = Vec::new();
    let table_props = map.get_or_create_container("props", LoroMap::new())?;
    sync_props(&table_props, &serde_json::to_value(&own)?)?;
    let list = map.get_or_create_container("rows", LoroMovableList::new())?;
    for (r, row) in rows.iter().enumerate() {
        let row_map = slot(&list, r)?;
        let cells = row_map.get_or_create_container("cells", LoroMovableList::new())?;
        for (c, cell) in row.iter().enumerate() {
            let cell_map = slot(&cells, c)?;
            let style = cell_map.get_or_create_container("props", LoroMap::new())?;
            sync_props(&style, &cell_json(&props.cell(r, c))?)?;
            let blocks = cell_map.get_or_create_container("blocks", LoroMovableList::new())?;
            write_blocks(&blocks, &cell.blocks)?;
        }
        truncate(&cells, row.len())?;
    }
    truncate(&list, rows.len())
}

/// Removes the table containers from a block that is no longer a table.
pub(crate) fn drop_table(map: &LoroMap) -> Result<(), WriteError> {
    for key in ["rows", "props"] {
        if map.get(key).is_some() {
            map.delete(key)?;
        }
    }
    Ok(())
}

/// Inserts a new row or cell map with a fresh id at `at`.
pub fn new_entry(list: &LoroMovableList, at: usize) -> Result<LoroMap, WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", fresh_id())?;
    Ok(map)
}

/// A cell's styling as JSON, without the spans of 1 a default cell has.
fn cell_json(style: &CellProps) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(style)?;
    if let Some(fields) = value.as_object_mut() {
        for span in ["colspan", "rowspan"] {
            if fields
                .get(span)
                .and_then(Value::as_u64)
                .is_some_and(|n| n <= 1)
            {
                fields.remove(span);
            }
        }
    }
    Ok(value)
}

fn slot(list: &LoroMovableList, at: usize) -> Result<LoroMap, WriteError> {
    if let Some(map) = block_map(list, at) {
        return Ok(map);
    }
    if at < list.len() {
        list.delete(at, 1)?;
    }
    new_entry(list, at)
}

fn truncate(list: &LoroMovableList, len: usize) -> Result<(), WriteError> {
    if list.len() > len {
        list.delete(len, list.len() - len)?;
    }
    Ok(())
}

pub fn canonical_table(rows: &mut [Vec<RichText>], props: &mut TableProps) {
    props.cells.truncate(rows.len());
    for (styles, row) in props.cells.iter_mut().zip(rows.iter()) {
        styles.truncate(row.len());
        for style in styles.iter_mut() {
            if style.colspan == 1 {
                style.colspan = 0;
            }
            if style.rowspan == 1 {
                style.rowspan = 0;
            }
        }
        while styles
            .last()
            .is_some_and(|style| *style == CellProps::default())
        {
            styles.pop();
        }
    }
    while props.cells.last().is_some_and(Vec::is_empty) {
        props.cells.pop();
    }
    for cell in rows.iter_mut().flatten() {
        cell.document = None;
        canonical(cell);
    }
}

/// Puts every table in `model` into the form it reads back from Loro.
pub fn canonical(model: &mut RichText) {
    for block in &mut model.blocks {
        if let BlockKind::Table { rows, props } = &mut block.kind {
            canonical_table(rows, props);
        }
    }
}

/// Writes a JSON object's fields as map entries, each value as its JSON
/// text, as the browser writes them. Keys the object lacks are removed.
pub(crate) fn sync_props(map: &LoroMap, object: &Value) -> Result<(), WriteError> {
    let fields = object.as_object().cloned().unwrap_or_default();
    for key in map
        .keys()
        .map(|key| key.as_str().to_owned())
        .collect::<Vec<_>>()
    {
        if !fields.contains_key(&key) {
            map.delete(&key)?;
        }
    }
    for (key, value) in &fields {
        let wanted = to_loro(value)?;
        let held = match map.get(key) {
            Some(ValueOrContainer::Value(held)) => Some(held),
            _ => None,
        };
        if held.as_ref() != Some(&wanted) {
            map.insert(key, wanted)?;
        }
    }
    Ok(())
}

pub(crate) fn to_loro(value: &Value) -> Result<LoroValue, WriteError> {
    Ok(serde_json::to_string(value)?.into())
}

/// Reads a props map: a string is JSON text (a bare string that is not JSON
/// reads as itself), and raw numbers and flags are accepted as they are.
/// Keys the model does not know, such as a cell's `header`, are ignored.
fn from_props<T: serde::de::DeserializeOwned>(map: &LoroMap) -> serde_json::Result<T> {
    let mut fields = Map::new();
    for key in map
        .keys()
        .map(|key| key.as_str().to_owned())
        .collect::<Vec<_>>()
    {
        let Some(ValueOrContainer::Value(value)) = map.get(&key) else {
            continue;
        };
        let value = match value {
            LoroValue::String(text) => {
                serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.to_string()))
            },
            LoroValue::I64(int) => Value::from(int),
            LoroValue::Double(float) => Value::from(float),
            LoroValue::Bool(flag) => Value::Bool(flag),
            _ => continue,
        };
        fields.insert(key, value);
    }
    serde_json::from_value(Value::Object(fields))
}
