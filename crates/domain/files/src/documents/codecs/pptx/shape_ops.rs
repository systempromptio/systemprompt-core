//! `move_shape`, `resize_shape` and `delete_shape` on a stored deck: the deck
//! is read as its model, the one shape changed, and the structure pass writes
//! the difference.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, EditOp};

use super::deck::model_of;
use super::projection;
use super::structure::restructure_package;
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckShape, DeckSlide};

fn shape_index(segment: &str) -> Option<usize> {
    segment
        .strip_prefix("shape[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

fn slide_index(segment: &str) -> Option<usize> {
    segment
        .strip_prefix("slide[")?
        .strip_suffix(']')?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}

fn locate<'d>(
    slides: &'d mut [DeckSlide],
    node: &DocumentNodeId,
) -> Result<(&'d mut DeckSlide, usize), CodecError> {
    let unknown = || CodecError::UnknownNode { node: node.clone() };
    let (slide, shape) = node.as_str().split_once('/').ok_or_else(unknown)?;
    let shape = shape_index(shape).ok_or_else(unknown)?;
    let slide = slide_index(slide)
        .and_then(|slide| slides.get_mut(slide))
        .filter(|slide| shape < slide.shapes.len())
        .ok_or_else(unknown)?;
    Ok((slide, shape))
}

fn set_frame(
    shape: &mut DeckShape,
    edit: impl FnOnce(&mut crate::documents::engine::loro::deck::Frame),
) -> Result<(), CodecError> {
    let frame = shape.frame.as_mut().ok_or(CodecError::Refused {
        detail: "this shape has no frame of its own to change".to_owned(),
    })?;
    edit(frame);
    Ok(())
}

pub(super) fn apply(package: &mut Package<'_>, op: &EditOp) -> Result<(), CodecError> {
    let mut target = model_of(&projection::load(package)?)?;
    match op {
        EditOp::MoveShape { node, x, y } => {
            let (slide, at) = locate(&mut target.slides, node)?;
            set_frame(&mut slide.shapes[at], |frame| {
                frame.x = *x;
                frame.y = *y;
            })?;
        },
        EditOp::ResizeShape { node, w, h } => {
            if *w <= 0 || *h <= 0 {
                return Err(CodecError::Refused {
                    detail: "a shape's width and height must be positive".to_owned(),
                });
            }
            let (slide, at) = locate(&mut target.slides, node)?;
            set_frame(&mut slide.shapes[at], |frame| {
                frame.w = *w;
                frame.h = *h;
            })?;
        },
        EditOp::DeleteShape { node } => {
            let (slide, at) = locate(&mut target.slides, node)?;
            slide.shapes.remove(at);
        },
        _ => {},
    }
    restructure_package(package, &target).map(|_| ())
}
