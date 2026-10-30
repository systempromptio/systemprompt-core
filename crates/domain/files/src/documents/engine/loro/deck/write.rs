//! Turning a deck document into a target [`DeckModel`].
//!
//! Slides, shapes and paragraphs are each matched by id within their list:
//! an entry that only moved is moved, an edited one has only its changed
//! fields rewritten and a removed one is deleted, so a write lands as the
//! smallest set of container operations a concurrent editor can merge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use crate::documents::model::Block;
use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText};

use super::super::child::{child_list, child_map};
use super::super::rich::{WriteError, block_map, insert_block, string_of, sync_text, text_of};
use super::super::write::{kind_json, sync_attrs};
use super::model::{DeckModel, DeckShape, DeckSlide, Frame};
use super::read::map_of;

pub fn write_deck(doc: &LoroDoc, target: &DeckModel) -> Result<(), WriteError> {
    sync_list(
        &super::slides(doc),
        &target.slides,
        |slide| slide.id.as_str(),
        insert_slide,
        sync_slide,
    )
}

pub fn insert_slide(
    list: &LoroMovableList,
    at: usize,
    slide: &DeckSlide,
) -> Result<(), WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", slide.id.as_str())?;
    if let Some(layout) = &slide.layout {
        map.insert("layout", layout.as_str())?;
    }
    let shapes = child_list(&map, "shapes")?;
    for (at, shape) in slide.shapes.iter().enumerate() {
        insert_shape(&shapes, at, shape)?;
    }
    Ok(())
}

pub fn insert_paragraph(
    list: &LoroMovableList,
    at: usize,
    block: &Block,
) -> Result<(), WriteError> {
    insert_block(list, at, block)
}

fn insert_shape(list: &LoroMovableList, at: usize, shape: &DeckShape) -> Result<(), WriteError> {
    let map = list.insert_container(at, LoroMap::new())?;
    map.insert("id", shape.id.as_str())?;
    map.insert("kind", shape.kind.name())?;
    if let Some(src) = &shape.src {
        map.insert("src", src.as_str())?;
    }
    if let Some(frame) = shape.frame {
        write_frame(&child_map(&map, "frame")?, frame)?;
    }
    let paragraphs = child_list(&map, "paragraphs")?;
    for (at, block) in shape.paragraphs.iter().enumerate() {
        insert_block(&paragraphs, at, block)?;
    }
    Ok(())
}

fn write_frame(map: &LoroMap, frame: Frame) -> Result<(), WriteError> {
    for (key, value) in [
        ("x", frame.x),
        ("y", frame.y),
        ("w", frame.w),
        ("h", frame.h),
    ] {
        let held = match map.get(key) {
            Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(held))) => Some(held),
            Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(held))) => {
                Some(held.round() as i64)
            },
            _ => None,
        };
        if held != Some(value) {
            map.insert(key, value)?;
        }
    }
    Ok(())
}

fn sync_slide(map: &LoroMap, slide: &DeckSlide) -> Result<(), WriteError> {
    sync_string(map, "layout", slide.layout.as_deref())?;
    let shapes = child_list(map, "shapes")?;
    sync_list(
        &shapes,
        &slide.shapes,
        |shape| shape.id.as_str(),
        insert_shape,
        sync_shape,
    )
}

fn sync_shape(map: &LoroMap, shape: &DeckShape) -> Result<(), WriteError> {
    sync_string(map, "kind", Some(shape.kind.name()))?;
    sync_string(map, "src", shape.src.as_deref())?;
    match (shape.frame, map_of(map, "frame")) {
        (Some(frame), Some(held)) => write_frame(&held, frame)?,
        (Some(frame), None) => write_frame(&child_map(map, "frame")?, frame)?,
        (None, Some(_)) => map.delete("frame")?,
        (None, None) => {},
    }
    let paragraphs = child_list(map, "paragraphs")?;
    sync_list(
        &paragraphs,
        &shape.paragraphs,
        |block| block.id.as_str(),
        insert_block,
        sync_paragraph,
    )
}

fn sync_paragraph(map: &LoroMap, block: &Block) -> Result<(), WriteError> {
    sync_string(map, "kind", Some(kind_json(&block.kind)?.as_str()))?;
    sync_attrs(map, &block.attrs)?;
    let text = match text_of(map) {
        Some(text) => text,
        None => map.insert_container("text", LoroText::new())?,
    };
    sync_text(&text, block)
}

fn sync_string(map: &LoroMap, key: &str, value: Option<&str>) -> Result<(), WriteError> {
    let held = string_of(map, key);
    match value {
        Some(value) if held.as_deref() != Some(value) => map.insert(key, value)?,
        None if held.is_some() => map.delete(key)?,
        _ => {},
    }
    Ok(())
}

fn sync_list<T>(
    list: &LoroMovableList,
    target: &[T],
    id: impl Fn(&T) -> &str,
    insert: impl Fn(&LoroMovableList, usize, &T) -> Result<(), WriteError>,
    sync: impl Fn(&LoroMap, &T) -> Result<(), WriteError>,
) -> Result<(), WriteError> {
    let wanted: HashSet<&str> = target.iter().map(&id).collect();
    let mut seen: HashSet<String> = HashSet::with_capacity(wanted.len());
    for at in (0..list.len()).rev() {
        let held = block_map(list, at).and_then(|map| string_of(&map, "id"));
        let keep = held
            .filter(|held| wanted.contains(held.as_str()))
            .is_some_and(|held| seen.insert(held));
        if !keep {
            list.delete(at, 1)?;
        }
    }
    for (at, item) in target.iter().enumerate() {
        let want = id(item);
        let from = (at..list.len()).find(|from| {
            block_map(list, *from)
                .and_then(|map| string_of(&map, "id"))
                .is_some_and(|held| held == want)
        });
        match from {
            Some(from) => {
                if from != at {
                    list.mov(from, at)?;
                }
                if let Some(map) = block_map(list, at) {
                    sync(&map, item)?;
                }
            },
            None => insert(list, at, item)?,
        }
    }
    Ok(())
}
