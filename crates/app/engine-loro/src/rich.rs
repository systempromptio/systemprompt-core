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
//!
//! Every replica must configure the same mark expansion before it edits
//! ([`configure`]); a browser client mirrors [`TEXT_STYLES`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

#[must_use]
pub fn read(doc: &LoroDoc) -> RichText {
    let list = blocks(doc);
    RichText {
        blocks: (0..list.len())
            .filter_map(|at| block_map(&list, at).and_then(|m| read_block(&m)))
            .collect(),
        document: read_settings(doc),
    }
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

fn read_block(map: &LoroMap) -> Option<Block> {
    let id = string_of(map, "id")?;
    let kind: BlockKind = serde_json::from_str(&string_of(map, "kind")?).ok()?;
    let attrs: ParaAttrs = string_of(map, "attrs")
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let (text, marks) = text_of(map)
        .map(|text| read_text(&text))
        .unwrap_or_default();
    Some(Block {
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
    if read_text(text).0 != block.text {
        text.update(&block.text, loro::UpdateOptions::default())?;
    }
    if read_text(text).1 != block.marks {
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
