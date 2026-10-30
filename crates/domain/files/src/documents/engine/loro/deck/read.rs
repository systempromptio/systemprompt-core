//! Reading a [`DeckModel`] out of a deck document's containers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind};
use loro::{Container, LoroDoc, LoroMap, LoroMovableList, LoroValue, ValueOrContainer};

use super::super::rich::{block_map, read_text, string_of, text_of};
use super::model::{DeckModel, DeckShape, DeckSlide, Frame, ShapeKind};

/// An entry of the deck that does not read. `path` names it as
/// `slide <n>`, `slide <n> shape <k>` or `... paragraph <j>`, zero-based.
#[derive(Debug, thiserror::Error)]
pub enum DeckReadError {
    #[error("{path} is not a map")]
    NotAMap { path: String },
    #[error("{path} has no id")]
    MissingId { path: String },
    #[error("{path} has no readable kind")]
    Kind { path: String },
    #[error("{path} has a frame without numeric x, y, w and h")]
    Frame { path: String },
}

pub fn read(doc: &LoroDoc) -> Result<DeckModel, DeckReadError> {
    let list = super::slides(doc);
    let slides = (0..list.len())
        .map(|at| {
            let path = format!("slide {at}");
            let map = block_map(&list, at)
                .ok_or_else(|| DeckReadError::NotAMap { path: path.clone() })?;
            read_slide(&path, &map)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DeckModel { slides })
}

pub(super) fn list_of(map: &LoroMap, key: &str) -> Option<LoroMovableList> {
    match map.get(key)? {
        ValueOrContainer::Container(Container::MovableList(list)) => Some(list),
        _ => None,
    }
}

pub(super) fn map_of(map: &LoroMap, key: &str) -> Option<LoroMap> {
    match map.get(key)? {
        ValueOrContainer::Container(Container::Map(map)) => Some(map),
        _ => None,
    }
}

fn number_of(map: &LoroMap, key: &str) -> Option<i64> {
    match map.get(key)? {
        ValueOrContainer::Value(LoroValue::I64(value)) => Some(value),
        ValueOrContainer::Value(LoroValue::Double(value)) if value.is_finite() => {
            Some(value.round() as i64)
        },
        _ => None,
    }
}

fn read_slide(path: &str, map: &LoroMap) -> Result<DeckSlide, DeckReadError> {
    let id = string_of(map, "id").ok_or_else(|| DeckReadError::MissingId {
        path: path.to_owned(),
    })?;
    let shapes = list_of(map, "shapes").map_or_else(
        || Ok(Vec::new()),
        |list| {
            (0..list.len())
                .map(|at| {
                    let path = format!("{path} shape {at}");
                    let shape = block_map(&list, at)
                        .ok_or_else(|| DeckReadError::NotAMap { path: path.clone() })?;
                    read_shape(&path, &shape)
                })
                .collect::<Result<Vec<_>, _>>()
        },
    )?;
    Ok(DeckSlide {
        id,
        layout: string_of(map, "layout"),
        shapes,
    })
}

fn read_shape(path: &str, map: &LoroMap) -> Result<DeckShape, DeckReadError> {
    let id = string_of(map, "id").ok_or_else(|| DeckReadError::MissingId {
        path: path.to_owned(),
    })?;
    let kind = string_of(map, "kind")
        .as_deref()
        .and_then(ShapeKind::from_name)
        .ok_or_else(|| DeckReadError::Kind {
            path: path.to_owned(),
        })?;
    let frame = match map_of(map, "frame") {
        None => None,
        Some(frame) => Some(read_frame(&frame).ok_or_else(|| DeckReadError::Frame {
            path: path.to_owned(),
        })?),
    };
    let paragraphs = list_of(map, "paragraphs").map_or_else(
        || Ok(Vec::new()),
        |list| {
            (0..list.len())
                .map(|at| read_paragraph(&format!("{path} paragraph {at}"), &list, at))
                .collect::<Result<Vec<_>, _>>()
        },
    )?;
    Ok(DeckShape {
        id,
        kind,
        frame,
        paragraphs,
    })
}

fn read_frame(map: &LoroMap) -> Option<Frame> {
    Some(Frame {
        x: number_of(map, "x")?,
        y: number_of(map, "y")?,
        w: number_of(map, "w")?,
        h: number_of(map, "h")?,
    })
}

fn read_paragraph(path: &str, list: &LoroMovableList, at: usize) -> Result<Block, DeckReadError> {
    let map = block_map(list, at).ok_or_else(|| DeckReadError::NotAMap {
        path: path.to_owned(),
    })?;
    let id = string_of(&map, "id").ok_or_else(|| DeckReadError::MissingId {
        path: path.to_owned(),
    })?;
    let kind: BlockKind = string_of(&map, "kind")
        .and_then(|json| serde_json::from_str(&json).ok())
        .ok_or_else(|| DeckReadError::Kind {
            path: path.to_owned(),
        })?;
    let attrs: ParaAttrs = string_of(&map, "attrs")
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let (text, marks) = text_of(&map)
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
