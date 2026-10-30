//! The structure pass of lowering a deck: shape frames, shapes added to or
//! removed from a slide, and slide order.
//!
//! Every slide is paired with its target as the text passes paired it, so by
//! now the deck holds exactly the target's slides. A moved or resized shape
//! has its `a:off`/`a:ext` rewritten (an `a:xfrm` is written into its
//! `p:spPr` when it inherited its frame), a paired text shape's paragraphs
//! take the target's marks and kinds ([`restyle`](super::restyle)), a removed
//! shape is cut from the `p:spTree`, an added text shape or image
//! ([`picture`](super::picture)) is placed in it, and `p:sldIdLst` is
//! rewritten in the target's order when that differs. When the target puts
//! the shapes in another z-order (the `p:spTree` order) the shapes are
//! written out again in that order; otherwise each is edited in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::deck::model_of;
use super::lower::{shape_pairs, slide_pairs};
use super::picture::picture_xml;
use super::projection::{self, Slide, shape_elements, sp_tree};
use super::restyle::paragraph_edits;
use super::zorder::{Placed, in_order, reordered};
use super::slides::{PRESENTATION, slide_refs};
use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckModel, DeckShape, DeckSlide, Frame, ShapeKind};
use crate::documents::model::BlockKind;

pub(super) fn restructure(bytes: &[u8], target: &DeckModel) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(bytes)?;
    let slides = projection::load(&mut package)?;
    let base = model_of(&slides)?;
    let pairs = slide_pairs(&base, target);
    let mut changed = false;
    for (t, paired) in pairs.iter().enumerate() {
        let Some(b) = *paired else { continue };
        let edited = slide_edits(&mut package, &slides[b], &base.slides[b], &target.slides[t])?;
        if let Some(xml) = edited {
            package.write(&slides[b].part, xml);
            changed = true;
        }
    }
    let order: Vec<usize> = pairs.iter().filter_map(|paired| *paired).collect();
    if order.len() == base.slides.len() && order.iter().enumerate().any(|(at, b)| at != *b) {
        reorder(&mut package, &order)?;
        changed = true;
    }
    if !changed {
        return Ok(bytes.to_vec());
    }
    package.finish()
}

fn slide_edits(
    package: &mut Package<'_>,
    slide: &Slide,
    base: &DeckSlide,
    target: &DeckSlide,
) -> Result<Option<String>, CodecError> {
    let root = parse_xml(&slide.part, &slide.xml)?;
    let Some(tree) = sp_tree(&root) else {
        return Ok(None);
    };
    let elements: Vec<&Element> = shape_elements(tree).collect();
    let pairs = shape_pairs(base, target);
    let mut kept = vec![false; base.shapes.len()];
    let mut edits = Vec::new();
    let mut placed = Vec::new();
    let mut next_id = max_id(&root) + 1;
    for (t, paired) in pairs.iter().enumerate() {
        let shape = &target.shapes[t];
        match *paired {
            Some(b) => {
                kept[b] = true;
                let Some(&element) = elements.get(b) else { continue };
                if let Some(frame) = shape.frame
                    && base.shapes[b].frame != Some(frame)
                {
                    edits.extend(frame_edits(element, &slide.xml, frame));
                }
                edits.extend(paragraph_edits(&slide.xml, element, &base.shapes[b], shape)?);
                placed.push(Placed::Kept(element));
            },
            None if shape.kind.holds_text() => {
                placed.push(Placed::Added(shape_xml(next_id, shape)));
                next_id += 1;
            },
            None if shape.kind == ShapeKind::Image => {
                if let Some(xml) = picture_xml(package, &slide.part, next_id, shape)? {
                    placed.push(Placed::Added(xml));
                    next_id += 1;
                }
            },
            None => {
                tracing::warn!(slide = %base.id, "only text and image shapes are added to a slide");
            },
        }
    }
    let removed: Vec<&Element> = kept
        .iter()
        .zip(&elements)
        .filter(|(kept, _)| !**kept)
        .map(|(_, element)| *element)
        .collect();
    if in_order(&placed) {
        for element in removed {
            edits.push((element.start, element.end, String::new()));
        }
        for added in placed {
            if let Placed::Added(xml) = added {
                edits.push((tree.inner_end, tree.inner_end, xml));
            }
        }
        return Ok((!edits.is_empty()).then(|| splice(&slide.xml, edits)));
    }
    Ok(Some(reordered(&slide.xml, &elements, &placed, edits)))
}

fn max_id(element: &Element) -> u64 {
    let own = if element.name == "p:cNvPr" {
        element
            .attr("id")
            .and_then(|id| id.parse().ok())
            .unwrap_or(0)
    } else {
        0
    };
    element.children.iter().map(max_id).fold(own, u64::max)
}

fn off_xml(frame: Frame) -> String {
    format!("<a:off x=\"{}\" y=\"{}\"/>", frame.x, frame.y)
}

fn ext_xml(frame: Frame) -> String {
    format!(
        "<a:ext cx=\"{}\" cy=\"{}\"/>",
        frame.w.max(0),
        frame.h.max(0)
    )
}

pub(super) fn xfrm_xml(frame: Frame) -> String {
    format!("<a:xfrm>{}{}</a:xfrm>", off_xml(frame), ext_xml(frame))
}

fn frame_edits(element: &Element, xml: &str, frame: Frame) -> Vec<(usize, usize, String)> {
    let sppr = element
        .child("p:spPr")
        .or_else(|| element.child("p:grpSpPr"));
    let xfrm = sppr
        .and_then(|pr| pr.child("a:xfrm"))
        .or_else(|| element.child("p:xfrm"));
    if let Some(xfrm) = xfrm {
        return match (xfrm.child("a:off"), xfrm.child("a:ext")) {
            (Some(off), Some(ext)) => vec![
                (off.start, off.end, off_xml(frame)),
                (ext.start, ext.end, ext_xml(frame)),
            ],
            _ => vec![(
                xfrm.inner_start,
                xfrm.inner_end,
                format!("{}{}", off_xml(frame), ext_xml(frame)),
            )],
        };
    }
    match sppr {
        Some(pr) if pr.raw(xml).ends_with("/>") => {
            vec![(
                pr.start,
                pr.end,
                format!("<{0}>{1}</{0}>", pr.name, xfrm_xml(frame)),
            )]
        },
        Some(pr) => vec![(pr.inner_start, pr.inner_start, xfrm_xml(frame))],
        None => Vec::new(),
    }
}

fn shape_xml(id: u64, shape: &DeckShape) -> String {
    let nv_pr = match shape.kind {
        ShapeKind::Title => "<p:nvPr><p:ph type=\"title\"/></p:nvPr>",
        ShapeKind::Body => "<p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr>",
        _ => "<p:nvPr/>",
    };
    let tx_box = if shape.kind == ShapeKind::Text {
        " txBox=\"1\""
    } else {
        ""
    };
    let geometry = shape.frame.map_or_else(String::new, |frame| {
        format!(
            "{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>",
            xfrm_xml(frame)
        )
    });
    let mut paragraphs: String = shape
        .paragraphs
        .iter()
        .map(|block| {
            paragraph_xml(
                &block.text,
                matches!(block.kind, BlockKind::ListItem { .. }),
            )
        })
        .collect();
    if paragraphs.is_empty() {
        paragraphs.push_str("<a:p><a:endParaRPr lang=\"en-US\"/></a:p>");
    }
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Shape {id}\"/><p:cNvSpPr{tx_box}/>{nv_pr}</p:nvSpPr><p:spPr>{geometry}</p:spPr><p:txBody><a:bodyPr wrap=\"square\"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"
    )
}

fn paragraph_xml(text: &str, bullet: bool) -> String {
    let mut out = String::from("<a:p>");
    if bullet {
        out.push_str("<a:pPr marL=\"285750\" indent=\"-285750\"><a:buChar char=\"•\"/></a:pPr>");
    }
    for (line, segment) in text.split('\n').enumerate() {
        if line > 0 {
            out.push_str("<a:br/>");
        }
        if !segment.is_empty() {
            out.push_str(&format!("<a:r><a:t>{}</a:t></a:r>", escape(segment)));
        }
    }
    out.push_str("<a:endParaRPr lang=\"en-US\"/></a:p>");
    out
}

fn reorder(package: &mut Package<'_>, order: &[usize]) -> Result<(), CodecError> {
    let refs = slide_refs(package)?;
    let xml = package.require(PRESENTATION)?;
    let root = parse_xml(PRESENTATION, &xml)?;
    let Some(list) = root.child("p:sldIdLst") else {
        return Ok(());
    };
    let entries: Option<String> = order
        .iter()
        .map(|b| refs.get(*b).map(|entry| &xml[entry.start..entry.end]))
        .collect();
    let Some(entries) = entries else {
        return Err(CodecError::Malformed {
            detail: "the slide order names a slide the presentation does not list",
        });
    };
    let rewritten = splice(&xml, vec![(list.inner_start, list.inner_end, entries)]);
    package.write(PRESENTATION, rewritten);
    Ok(())
}
