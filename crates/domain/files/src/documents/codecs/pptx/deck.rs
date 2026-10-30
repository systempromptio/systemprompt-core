//! A deck read as the engine's [`DeckModel`].
//!
//! A slide's id is its part name, a shape's id its `cNvPr` id (its index on
//! the slide when that is absent or repeated) and a paragraph's id is
//! `<slide part>#<shape id>/p<index>`. Frames come from the shape's own
//! `xfrm`; a shape that inherits its frame from its layout has none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_provider_contracts::documents::CodecError;

use super::projection::{self, Content, Shape, Slide, shape_elements, sp_tree};
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckModel, DeckShape, DeckSlide, Frame, ShapeKind};
use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind};

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
            shape_of(&slide.part, id, shape, element.and_then(frame_of))
        })
        .collect();
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

fn shape_of(part: &str, id: String, shape: &Shape, frame: Option<Frame>) -> DeckShape {
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
                marks: Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    DeckShape {
        id,
        kind: kind_of(shape),
        frame,
        paragraphs,
    }
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
