//! The `RichText` model held in Loro containers.
//!
//! The root [`LoroMovableList`] named [`BLOCKS_CONTAINER`] holds one
//! [`LoroMap`] per block: `id` (string), `kind` (the block's type and
//! attributes as JSON, see [`BlockKind`]) and `text` (a [`LoroText`] whose
//! marks are the block's marks). Flag marks are stored as `true`, valued
//! marks as their string. Table cells travel inside `kind`, so a cell edit
//! replaces its table's `kind`; per-cell merging arrives with the table ops.
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
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};

pub const BLOCKS_CONTAINER: &str = "blocks";

/// Each mark key and how it expands when text is typed at its edge.
pub const TEXT_STYLES: [(&str, ExpandType); 9] = [
    ("bold", ExpandType::After),
    ("italic", ExpandType::After),
    ("underline", ExpandType::After),
    ("strike", ExpandType::After),
    ("code", ExpandType::None),
    ("link", ExpandType::None),
    ("color", ExpandType::After),
    ("highlight", ExpandType::After),
    ("comment", ExpandType::None),
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

/// Reads the model; a block whose map is malformed is skipped.
#[must_use]
pub fn read(doc: &LoroDoc) -> RichText {
    let list = blocks(doc);
    RichText {
        blocks: (0..list.len())
            .filter_map(|at| block_map(&list, at).and_then(|m| read_block(&m)))
            .collect(),
    }
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
    let (text, marks) = text_of(map)
        .map(|text| read_text(&text))
        .unwrap_or_default();
    Some(Block {
        id,
        kind,
        text,
        marks,
    })
}

/// The text and its marks, merging adjacent runs of one mark.
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

/// Writes a new block's map at `at`.
pub fn insert_block(list: &LoroMovableList, at: usize, block: &Block) -> Result<(), LoroError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", block.id.as_str())?;
    map.insert("kind", kind_json(&block.kind))?;
    let text = map.insert_container("text", LoroText::new())?;
    text.insert(0, &block.text)?;
    mark_all(&text, &block.marks)
}

pub(crate) fn kind_json(kind: &BlockKind) -> String {
    serde_json::to_string(kind).unwrap_or_default()
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

/// Rewrites `text` and its marks to `block`'s when they differ.
pub fn sync_text(text: &LoroText, block: &Block) -> Result<(), LoroError> {
    if read_text(text).0 != block.text {
        text.update(&block.text, loro::UpdateOptions::default())
            .map_err(|_| LoroError::NotImplemented("text update timed out"))?;
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
