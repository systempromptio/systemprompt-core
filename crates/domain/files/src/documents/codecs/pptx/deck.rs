//! A deck read as the engine's [`DeckModel`].
//!
//! A slide's id is its part name, a shape's id its `cNvPr` id (its index on
//! the slide when that is absent or repeated) and a paragraph's id is
//! `<slide part>#<shape id>/p<index>`. Frames come from the shape's own
//! `xfrm`, or for a placeholder without one, the frame it inherits from its
//! layout or master ([`inherit`](super::inherit)). A
//! paragraph's marks are read from its runs ([`runs`](super::runs)).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_provider_contracts::documents::CodecError;

use super::inherit::{Rect, inherited};
use super::projection::{self, Content, Shape, Slide, shape_elements, sp_tree};
use super::runs::read_marks;
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckModel, DeckShape, DeckSlide, Frame, ShapeKind};
use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind, Mark};

pub fn deck_model(bytes: &[u8]) -> Result<DeckModel, CodecError> {
    model_of(&projection::load(&mut Package::open(bytes)?)?)
}

pub(super) fn model_of(slides: &[Slide]) -> Result<DeckModel, CodecError> {
    slides
        .iter()
        .map(slide_of)
        .collect::<Result<Vec<_>, _>>()
        .map(|slides| DeckModel { slides })
}

fn slide_of(slide: &Slide) -> Result<DeckSlide, CodecError> {
    let root = parse_xml(&slide.part, &slide.xml)?;
    let elements: Vec<&Element> =
        sp_tree(&root).map_or_else(Vec::new, |tree| shape_elements(tree).collect());
    let mut seen = HashSet::new();
    let shapes = slide
        .shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let element = elements.get(index).copied();
            let id = element
                .and_then(|el| el.find("p:cNvPr"))
                .and_then(|el| el.attr("id"))
                .map(str::to_owned)
                .filter(|id| seen.insert(id.clone()))
                .unwrap_or_else(|| format!("shape-{index}"));
            let marks = element.map_or_else(|| Ok(Vec::new()), |el| paragraph_marks(slide, el))?;
            let frame = element.and_then(|el| {
                frame_of(el).or_else(|| inherited(&slide.frames, el).map(rect_frame))
            });
            let mut built = shape_of(&slide.part, id, shape, frame, marks);
            if let Content::Image(rel) = &shape.content {
                built.src = slide
                    .images
                    .iter()
                    .find(|(id, _)| id == rel)
                    .map(|(_, part)| part.clone());
            }
            Ok(built)
        })
        .collect::<Result<_, CodecError>>()?;
    Ok(DeckSlide {
        id: slide.part.clone(),
        layout: slide.layout.clone(),
        shapes,
    })
}

fn kind_of(shape: &Shape) -> ShapeKind {
    match (&shape.content, shape.placeholder.as_deref()) {
        (Content::Text(_), _) if shape.is_title() => ShapeKind::Title,
        (Content::Text(_), Some("body" | "obj" | "subTitle")) => ShapeKind::Body,
        (Content::Text(_), _) => ShapeKind::Text,
        (Content::Table(_), _) => ShapeKind::Table,
        (Content::Image(_), _) => ShapeKind::Image,
        (Content::Other, _) => ShapeKind::Other,
    }
}

fn paragraph_marks(slide: &Slide, element: &Element) -> Result<Vec<Vec<Mark>>, CodecError> {
    element
        .child("p:txBody")
        .map_or_else(Vec::new, |body| body.children_named("a:p").collect())
        .into_iter()
        .map(|p| read_marks(&slide.part, &slide.xml, p))
        .collect()
}

fn shape_of(
    part: &str,
    id: String,
    shape: &Shape,
    frame: Option<Frame>,
    mut marks: Vec<Vec<Mark>>,
) -> DeckShape {
    let paragraphs = match &shape.content {
        Content::Text(paras) => paras
            .iter()
            .enumerate()
            .map(|(index, para)| Block {
                id: format!("{part}#{id}/p{index}"),
                kind: if para.bullet {
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
                text: para.text.clone(),
                marks: marks.get_mut(index).map(std::mem::take).unwrap_or_default(),
            })
            .collect(),
        _ => Vec::new(),
    };
    DeckShape {
        id,
        kind: kind_of(shape),
        frame,
        paragraphs,
        src: None,
    }
}

const fn rect_frame((x, y, w, h): Rect) -> Frame {
    Frame { x, y, w, h }
}

fn frame_of(element: &Element) -> Option<Frame> {
    let xfrm = element
        .child("p:spPr")
        .and_then(|pr| pr.child("a:xfrm"))
        .or_else(|| element.child("p:xfrm"))
        .or_else(|| element.child("p:grpSpPr").and_then(|pr| pr.child("a:xfrm")))?;
    let off = xfrm.child("a:off")?;
    let ext = xfrm.child("a:ext")?;
    let number = |el: &Element, key: &str| el.attr(key)?.parse::<i64>().ok();
    Some(Frame {
        x: number(off, "x")?,
        y: number(off, "y")?,
        w: number(ext, "cx")?,
        h: number(ext, "cy")?,
    })
}
