//! Builders for the blocks and text shapes a live deck op inserts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::engine::loro::deck::{DeckShape, ShapeKind};
use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind};

pub(super) fn new_block(text: &str, list_item: bool) -> Block {
    Block {
        id: uuid::Uuid::new_v4().to_string(),
        kind: if list_item {
            BlockKind::ListItem {
                level: 0,
                ordered: false,
                numbering: None,
                checked: None,
            }
        } else {
            BlockKind::Paragraph
        },
        attrs: ParaAttrs::default(),
        text: text.to_owned(),
        marks: Vec::new(),
    }
}

pub(super) fn text_shape(kind: ShapeKind, lines: &[String], list_item: bool) -> DeckShape {
    DeckShape {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        frame: None,
        src: None,
        paragraphs: lines
            .iter()
            .map(|line| new_block(line, list_item))
            .collect(),
    }
}
