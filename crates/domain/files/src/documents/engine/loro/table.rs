//! A table block held as Loro containers, so concurrent edits to different
//! cells, rows or properties merge instead of replacing the table.
//!
//! The block map keeps `kind` = [`TABLE_KIND`] and adds `props`, a map of
//! the table's own properties (`columns` as JSON text, `header_rows` as an
//! i64, `style` as a string), and `rows`, a movable list of row maps `{id,
//! cells}`. `cells` is a movable list of cell maps `{id, props, blocks}`:
//! `props` holds the cell's `shade`, `align` and `valign` as strings and
//! `colspan` and `rowspan` as i64 (absent when default; a span of 1 is never
//! written), and `blocks` is a movable list of block maps in the same shape
//! as the root list. A browser editor mirrors this shape exactly; reading
//! also accepts the earlier all-JSON-text encoding.
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

use super::child::{fresh_list, fresh_map, is_legacy};
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
        .map_err(|_malformed| bad())?
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
                .map_err(|_malformed| bad())?
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
    let table_props = fresh_map(map, "props")?;
    sync_props(&table_props, &serde_json::to_value(&own)?)?;
    let ids = legacy_ids(map);
    let list = fresh_list(map, "rows")?;
    for (r, row) in rows.iter().enumerate() {
        let row_map = slot(&list, r, ids.get(r).map(|(id, _)| id.as_str()))?;
        let cell_ids = match ids.get(r) {
            Some((_, cells)) => cells.clone(),
            None => legacy_cell_ids(&row_map),
        };
        let cells = fresh_list(&row_map, "cells")?;
        for (c, cell) in row.iter().enumerate() {
            let cell_map = slot(&cells, c, cell_ids.get(c).map(String::as_str))?;
            let style = fresh_map(&cell_map, "props")?;
            sync_props(&style, &cell_json(&props.cell(r, c))?)?;
            let blocks = fresh_list(&cell_map, "blocks")?;
            write_blocks(&blocks, &cell.blocks)?;
        }
        truncate(&cells, row.len())?;
    }
    truncate(&list, rows.len())
}

fn legacy_ids(table: &LoroMap) -> Vec<(String, Vec<String>)> {
    let Some(rows) = list_of(table, "rows").filter(|_| is_legacy(table, "rows")) else {
        return Vec::new();
    };
    (0..rows.len())
        .filter_map(|r| block_map(&rows, r))
        .map(|row| {
            let id = string_of(&row, "id").unwrap_or_else(fresh_id);
            let cells = list_of(&row, "cells")
                .map(|cells| ids_of(&cells))
                .unwrap_or_default();
            (id, cells)
        })
        .collect()
}

fn legacy_cell_ids(row: &LoroMap) -> Vec<String> {
    list_of(row, "cells")
        .filter(|_| is_legacy(row, "cells"))
        .map(|cells| ids_of(&cells))
        .unwrap_or_default()
}

fn ids_of(list: &LoroMovableList) -> Vec<String> {
    (0..list.len())
        .filter_map(|at| block_map(list, at))
        .map(|cell| string_of(&cell, "id").unwrap_or_else(fresh_id))
        .collect()
}

pub(crate) fn drop_table(map: &LoroMap) -> Result<(), WriteError> {
    for key in ["rows", "props"] {
        if map.get(key).is_some() {
            map.delete(key)?;
        }
    }
    Ok(())
}

pub fn new_entry(list: &LoroMovableList, at: usize) -> Result<LoroMap, WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", fresh_id())?;
    Ok(map)
}

// JSON: a cell's props as the browser's untyped Loro map entries.
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

fn slot(list: &LoroMovableList, at: usize, id: Option<&str>) -> Result<LoroMap, WriteError> {
    if let Some(map) = block_map(list, at) {
        return Ok(map);
    }
    if at < list.len() {
        list.delete(at, 1)?;
    }
    match id {
        Some(id) => {
            let map = list.insert_container(at, LoroMap::new())?;
            map.insert("id", id)?;
            Ok(map)
        },
        None => new_entry(list, at),
    }
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

pub fn canonical(model: &mut RichText) {
    for block in &mut model.blocks {
        if let BlockKind::Table { rows, props } = &mut block.kind {
            canonical_table(rows, props);
        }
    }
}

// JSON: an object written field by field into a Loro map, as the browser does.
#[expect(
    clippy::needless_collect,
    reason = "the keys are snapshotted before entries are deleted from the map"
)]
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
        let wanted = to_loro(key, value)?;
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

// JSON: one map field converted to its native Loro value.
pub(crate) fn to_loro(key: &str, value: &Value) -> Result<LoroValue, WriteError> {
    Ok(match value {
        _ if key == JSON_KEY => serde_json::to_string(value)?.into(),
        Value::Bool(flag) => (*flag).into(),
        Value::Number(number) => number
            .as_i64()
            .map_or_else(|| number.as_f64().unwrap_or_default().into(), Into::into),
        Value::String(text) => text.as_str().into(),
        Value::Null => LoroValue::Null,
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)?.into(),
    })
}

const JSON_KEY: &str = "columns";

const NUMBER_KEYS: [&str; 3] = ["header_rows", "colspan", "rowspan"];

fn from_props<T: serde::de::DeserializeOwned>(map: &LoroMap) -> serde_json::Result<T> {
    let mut fields = Map::new();
    for key in map.keys().map(|key| key.as_str().to_owned()) {
        let Some(ValueOrContainer::Value(value)) = map.get(&key) else {
            continue;
        };
        let value = match value {
            LoroValue::String(text)
                if key == JSON_KEY
                    || NUMBER_KEYS.contains(&key.as_str())
                    || text.starts_with('"') =>
            {
                serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.to_string()))
            },
            LoroValue::String(text) => Value::String(text.to_string()),
            LoroValue::I64(int) => Value::from(int),
            LoroValue::Double(float) => Value::from(float),
            LoroValue::Bool(flag) => Value::Bool(flag),
            _ => continue,
        };
        fields.insert(key, value);
    }
    serde_json::from_value(Value::Object(fields))
}
