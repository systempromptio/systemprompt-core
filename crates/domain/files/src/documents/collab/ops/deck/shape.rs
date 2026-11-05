//! The shape a `slide[N]/shape[S]` node names in a live deck, and a write to
//! its frame.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroMovableList};

use super::live::{DeckOpError, Node, child_list, node, unknown};
use crate::documents::engine::loro::deck::slides;
use crate::documents::engine::loro::rich::block_map;

pub(super) fn shape_at(doc: &LoroDoc, id: &str) -> Result<(LoroMovableList, usize), DeckOpError> {
    let Node::Shape(slide, shape) = node(id)? else {
        return Err(unknown(id));
    };
    let shapes = block_map(&slides(doc), slide)
        .and_then(|map| child_list(&map, "shapes"))
        .filter(|shapes| shape < shapes.len())
        .ok_or_else(|| unknown(id))?;
    Ok((shapes, shape))
}

// Written per property, as the browser does, so a person moving the shape and
// an agent resizing it both land.
pub(super) fn set_frame(
    doc: &LoroDoc,
    id: &str,
    values: &[(&str, i64)],
) -> Result<Vec<String>, DeckOpError> {
    let (shapes, at) = shape_at(doc, id)?;
    let shape = block_map(&shapes, at).ok_or_else(|| unknown(id))?;
    let frame = shape.ensure_mergeable_map("frame")?;
    for (key, value) in values {
        frame.insert(key, *value)?;
    }
    Ok(Vec::new())
}
