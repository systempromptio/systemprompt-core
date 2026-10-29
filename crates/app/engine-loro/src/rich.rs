//! The `RichText` model held in Loro containers.
//!
//! The root [`LoroMovableList`] named [`BLOCKS_CONTAINER`] holds one
//! [`LoroMap`] per block: `id` (string), `kind` (the block's type and
//! attributes as JSON, see [`BlockKind`]), `attrs` (its paragraph
//! attributes as JSON, absent when default) and `text` (a [`LoroText`]
//! whose marks are the block's marks). Flag marks are stored as `true`,
//! valued marks as their string. Table cells, table geometry and section
//! page setup travel inside `kind`, so a cell edit replaces its table's
//! `kind`. The document's styles, theme, numbering and final section live
//! as one JSON string under `settings` in the root map [`DOCUMENT_CONTAINER`].
//! [`read`] fails with a [`ReadError`] on a block it cannot read.
//!
//! Every replica must configure the same mark expansion before it edits
//! ([`configure`]); a browser client mirrors [`TEXT_STYLES`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use loro::{
    Container, ExpandType, LoroDoc, LoroError, LoroMap, LoroMovableList, LoroText, LoroValue,
    StyleConfig, StyleConfigMap, TextDelta, ValueOrContainer,
};
use systemprompt_files::documents::model::layout::ParaAttrs;
use systemprompt_files::documents::model::{
    Block, BlockKind, DocumentSettings, Mark, MarkKind, RichText,
};

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
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABlock { at } => write!(f, "list entry {at} is not a block"),
            Self::MissingId { at } => write!(f, "block {at} has no id"),
            Self::MissingKind { id } => write!(f, "block {id} has no kind"),
            Self::Kind { id, .. } => write!(f, "block {id} has an unreadable kind"),
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
    let list = blocks(doc);
    let blocks = (0..list.len())
        .map(|at| {
            let map = block_map(&list, at).ok_or(ReadError::NotABlock { at })?;
            read_block(at, &map)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RichText {
        blocks,
        document: read_settings(doc),
    })
}

fn read_settings(doc: &LoroDoc) -> Option<Box<DocumentSettings>> {
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

pub(crate) fn text_of(map: &LoroMap) -> Option<LoroText> {
    match map.get("text")? {
        ValueOrContainer::Container(Container::Text(text)) => Some(text),
        _ => None,
    }
}

fn read_block(at: usize, map: &LoroMap) -> Result<Block, ReadError> {
    let id = string_of(map, "id").ok_or(ReadError::MissingId { at })?;
    let Some(kind) = string_of(map, "kind") else {
        return Err(ReadError::MissingKind { id });
    };
    let kind: BlockKind = match serde_json::from_str(&kind) {
        Ok(kind) => kind,
        Err(source) => return Err(ReadError::Kind { id, source }),
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

pub fn insert_block(list: &LoroMovableList, at: usize, block: &Block) -> Result<(), WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", block.id.as_str())?;
    map.insert("kind", kind_json(&block.kind)?)?;
    sync_attrs(&map, &block.attrs)?;
    let text = map.insert_container("text", LoroText::new())?;
    text.insert(0, &block.text)?;
    mark_all(&text, &block.marks)?;
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
