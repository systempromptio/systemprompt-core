//! Builders for the blocks and text shapes a live deck op inserts.
//!
//! An inserted title or body takes the frame of the first shape of its kind
//! the deck already holds, or the standard 16:9 placeholder frame when there
//! is none, so a new slide's title and body never share one frame.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::LoroDoc;

use crate::documents::engine::loro::deck::{DeckShape, Frame, ShapeKind, read};
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

const TITLE_FRAME: Frame = Frame {
    x: 838_200,
    y: 365_125,
    w: 10_515_600,
    h: 1_325_563,
};
const BODY_FRAME: Frame = Frame {
    x: 838_200,
    y: 1_825_625,
    w: 10_515_600,
    h: 4_351_338,
};

pub(super) fn frame_for(doc: &LoroDoc, kind: ShapeKind) -> Option<Frame> {
    let held = read(doc).ok().and_then(|deck| {
        deck.slides
            .iter()
            .flat_map(|slide| &slide.shapes)
            .find(|shape| shape.kind == kind)
            .and_then(|shape| shape.frame)
    });
    held.or(match kind {
        ShapeKind::Title => Some(TITLE_FRAME),
        ShapeKind::Body => Some(BODY_FRAME),
        _ => None,
    })
}

pub(super) fn text_shape(
    kind: ShapeKind,
    frame: Option<Frame>,
    lines: &[String],
    list_item: bool,
) -> DeckShape {
    DeckShape {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        frame,
        src: None,
        paragraphs: lines
            .iter()
            .map(|line| new_block(line, list_item))
            .collect(),
    }
}
