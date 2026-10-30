//! The `RichText` model held in Loro containers.
//!
//! The root [`LoroMovableList`] named [`BLOCKS_CONTAINER`] holds one
//! [`LoroMap`] per block: `id` (string), `kind` (the block's type and
//! attributes as JSON, see [`BlockKind`]), `attrs` (its paragraph
//! attributes as JSON, absent when default) and `text` (a [`LoroText`]
//! whose marks are the block's marks). Flag marks are stored as `true`,
//! valued marks as their string. A table keeps only `{"type":"table"}` in
//! `kind` and holds its geometry and cells in containers (see
//! [`super::table`]), so a cell edit merges with its neighbours'. A table
//! written before that shape keeps its cells inside `kind`; it still reads,
//! and [`has_legacy_tables`] finds it for an upgrade. Section page setup
//! travels inside `kind`. The document's styles, theme, numbering and final
//! section live as one JSON string under `settings` in the root map
//! [`DOCUMENT_CONTAINER`]. [`read`] fails with a [`ReadError`] on a block it
//! cannot read.
//!
//! Every replica must configure the same mark expansion before it edits
//! ([`configure`]); a browser client mirrors [`TEXT_STYLES`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind, DocumentSettings, Mark, MarkKind, RichText};
use loro::{
    Container, ExpandType, LoroDoc, LoroMap, LoroMovableList, LoroText, LoroValue, StyleConfig,
    StyleConfigMap, TextDelta, ValueOrContainer,
};

use super::table::{TABLE_KIND, canonical_table, is_table_map, read_table};
pub use super::write::{insert_block, sync_text};

pub const BLOCKS_CONTAINER: &str = "blocks";

pub const DOCUMENT_CONTAINER: &str = "document";

pub type WriteError = Box<dyn std::error::Error + Send + Sync>;

/// A block in [`BLOCKS_CONTAINER`] that does not read as a model block.
///
/// Reading refuses the whole document rather than leaving the block out, so
/// a checkpoint never lands a body that silently lost content.
#[derive(Debug)]
pub enum ReadError {
    NotABlock {
        at: usize,
    },
    MissingId {
        at: usize,
    },
    MissingKind {
        id: String,
    },
    Kind {
        id: String,
        source: serde_json::Error,
    },
    Table {
        id: String,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABlock { at } => write!(f, "list entry {at} is not a block"),
            Self::MissingId { at } => write!(f, "block {at} has no id"),
            Self::MissingKind { id } => write!(f, "block {id} has no kind"),
            Self::Kind { id, .. } => write!(f, "block {id} has an unreadable kind"),
            Self::Table { id } => write!(f, "table {id} has an unreadable row or cell"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Kind { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub const TEXT_STYLES: [(&str, ExpandType); 16] = [
    ("bold", ExpandType::After),
    ("italic", ExpandType::After),
    ("underline", ExpandType::After),
    ("strike", ExpandType::After),
    ("code", ExpandType::None),
    ("link", ExpandType::None),
    ("color", ExpandType::After),
    ("highlight", ExpandType::After),
    ("comment", ExpandType::None),
    ("font", ExpandType::After),
    ("size", ExpandType::After),
    ("superscript", ExpandType::After),
    ("subscript", ExpandType::After),
    ("caps", ExpandType::After),
    ("field", ExpandType::None),
    ("footnote", ExpandType::None),
];

pub fn configure(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    for (key, expand) in TEXT_STYLES {
        styles.insert(key.into(), StyleConfig { expand });
    }
    doc.config_text_style(styles);
}

#[must_use]
pub fn blocks(doc: &LoroDoc) -> LoroMovableList {
    doc.get_movable_list(BLOCKS_CONTAINER)
}

pub fn read(doc: &LoroDoc) -> Result<RichText, ReadError> {
    Ok(RichText {
        blocks: read_blocks(&blocks(doc))?,
        document: read_settings(doc),
    })
}

pub(crate) fn read_blocks(list: &LoroMovableList) -> Result<Vec<Block>, ReadError> {
    (0..list.len())
        .map(|at| {
            let map = block_map(list, at).ok_or(ReadError::NotABlock { at })?;
            read_block(at, &map)
        })
        .collect()
}

/// Whether a top-level table still keeps its cells inside `kind`.
#[must_use]
pub fn has_legacy_tables(doc: &LoroDoc) -> bool {
    let list = blocks(doc);
    (0..list.len()).any(|at| {
        block_map(&list, at).is_some_and(|map| {
            !is_table_map(&map)
                && string_of(&map, "kind").is_some_and(|kind| {
                    kind != TABLE_KIND && kind.starts_with("{\"type\":\"table\"")
                })
        })
    })
}

pub(crate) fn read_settings(doc: &LoroDoc) -> Option<Box<DocumentSettings>> {
    let json = string_of(&doc.get_map(DOCUMENT_CONTAINER), "settings")?;
    match serde_json::from_str(&json) {
        Ok(settings) => Some(Box::new(settings)),
        Err(error) => {
            tracing::warn!(%error, "live document settings unreadable; the base settings are kept");
            None
        },
    }
}

pub fn sync_settings(doc: &LoroDoc, settings: Option<&DocumentSettings>) -> Result<(), WriteError> {
    let map = doc.get_map(DOCUMENT_CONTAINER);
    let held = string_of(&map, "settings");
    match settings.filter(|settings| !settings.is_default()) {
        Some(settings) => {
            let json = serde_json::to_string(settings)?;
            if held.as_deref() != Some(json.as_str()) {
                map.insert("settings", json)?;
            }
        },
        None if held.is_some() => map.delete("settings")?,
        None => {},
    }
    Ok(())
}

pub(crate) fn block_map(list: &LoroMovableList, at: usize) -> Option<LoroMap> {
    match list.get(at)? {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

pub(crate) fn string_of(map: &LoroMap, key: &str) -> Option<String> {
    match map.get(key)? {
        ValueOrContainer::Value(LoroValue::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

pub(crate) fn list_of(map: &LoroMap, key: &str) -> Option<LoroMovableList> {
    match map.get(key)? {
        ValueOrContainer::Container(Container::MovableList(list)) => Some(list),
        _ => None,
    }
}

pub(crate) fn map_of(map: &LoroMap, key: &str) -> Option<LoroMap> {
    match map.get(key)? {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

pub(crate) fn text_of(map: &LoroMap) -> Option<LoroText> {
    match map.get("text")? {
        ValueOrContainer::Container(Container::Text(text)) => Some(text),
        _ => None,
    }
}

pub(crate) fn read_block(at: usize, map: &LoroMap) -> Result<Block, ReadError> {
    let id = string_of(map, "id").ok_or(ReadError::MissingId { at })?;
    let Some(kind) = string_of(map, "kind") else {
        return Err(ReadError::MissingKind { id });
    };
    let kind = if is_table_map(map) {
        read_table(&id, map)?
    } else {
        match serde_json::from_str(&kind) {
            Ok(BlockKind::Table {
                mut rows,
                mut props,
            }) => {
                canonical_table(&mut rows, &mut props);
                BlockKind::Table { rows, props }
            },
            Ok(kind) => kind,
            Err(source) => return Err(ReadError::Kind { id, source }),
        }
    };
    let attrs: ParaAttrs = string_of(map, "attrs")
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let (text, marks) = text_of(map)
        .map(|text| read_text(&text))
        .unwrap_or_default();
    Ok(Block {
        id,
        kind,
        attrs,
        text,
        marks,
    })
}

#[must_use]
pub fn read_text(text: &LoroText) -> (String, Vec<Mark>) {
    let mut out = String::new();
    let mut at = 0;
    let mut marks: Vec<Mark> = Vec::new();
    for delta in text.to_delta() {
        let TextDelta::Insert { insert, attributes } = delta else {
            continue;
        };
        let len = insert.chars().count();
        for (key, value) in attributes.into_iter().flatten() {
            let value = match value {
                LoroValue::String(value) => value.to_string(),
                LoroValue::Null => continue,
                _ => String::new(),
            };
            let Some(kind) = MarkKind::from_key(&key, &value) else {
                continue;
            };
            match marks.iter_mut().find(|m| m.end == at && m.kind == kind) {
                Some(open) => open.end = at + len,
                None => marks.push(Mark {
                    start: at,
                    end: at + len,
                    kind,
                }),
            }
        }
        out.push_str(&insert);
        at += len;
    }
    marks.sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
    (out, marks)
}
