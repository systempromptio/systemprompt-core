//! Shape z-order on a slide: the order of the shapes in its `p:spTree`.
//!
//! The structure pass places every target shape, kept or added, in the
//! target's order. When that order is the slide's own with additions at the
//! end, the shapes are edited in place; otherwise [`reordered`] writes the
//! shapes out again in the target's order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::{Element, splice};

pub(super) enum Placed<'e> {
    Kept(&'e Element),
    Added(String),
}

pub(super) fn in_order(placed: &[Placed<'_>]) -> bool {
    let mut last = 0;
    let mut added = false;
    for entry in placed {
        match entry {
            Placed::Kept(element) if added || element.start < last => return false,
            Placed::Kept(element) => last = element.start,
            Placed::Added(_) => added = true,
        }
    }
    true
}

pub(super) fn reordered(
    xml: &str,
    elements: &[&Element],
    placed: &[Placed<'_>],
    edits: Vec<(usize, usize, String)>,
) -> String {
    let (Some(first), Some(last)) = (elements.first(), elements.last()) else {
        return splice(xml, edits);
    };
    let own = |element: &Element| {
        let local = edits
            .iter()
            .filter(|(start, end, _)| *start >= element.start && *end <= element.end)
            .map(|(start, end, text)| (start - element.start, end - element.start, text.clone()))
            .collect();
        splice(element.raw(xml), local)
    };
    let mut body: String = placed
        .iter()
        .map(|entry| match entry {
            Placed::Kept(element) => own(element),
            Placed::Added(shape) => shape.clone(),
        })
        .collect();
    let mut cursor = first.end;
    for element in &elements[1..] {
        body.push_str(xml[cursor..element.start].trim());
        cursor = element.end;
    }
    splice(xml, vec![(first.start, last.end, body)])
}
