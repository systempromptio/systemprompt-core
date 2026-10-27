//! Applies edit operations to a deck by splicing the touched paragraphs of a
//! slide part, or by rewriting the presentation parts for a slide insert or
//! removal.
//!
//! Every anchored operation is checked against the deck as read before any
//! operation runs, so a batch applies all or none. Node ids in later
//! operations of a batch resolve against the deck as edited by the earlier
//! ones.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, EditOp, NodeKind, Projection, content_hash,
};

use super::layout::NewSlide;
use super::projection::{self, Content, Paragraph, Slide};
use super::{nodes, slides};
use crate::documents::codecs::xml_tree::{parse_xml, splice};
use crate::documents::codecs::zip_package::Package;

enum Target {
    Slide(usize),
    Paragraph {
        slide: usize,
        shape: usize,
        para: usize,
    },
    Other,
}

fn index(segment: &str, prefix: &str) -> Option<usize> {
    segment
        .strip_prefix(prefix)?
        .strip_suffix(']')?
        .parse()
        .ok()
}

fn target(node: &DocumentNodeId) -> Result<Target, CodecError> {
    let unknown = || CodecError::UnknownNode { node: node.clone() };
    let mut segments = node.as_str().split('/');
    let slide = segments
        .next()
        .and_then(|s| index(s, "slide["))
        .filter(|n| *n > 0)
        .ok_or_else(unknown)?;
    let rest: Vec<&str> = segments.collect();
    Ok(match rest.as_slice() {
        [] => Target::Slide(slide),
        [shape, para] => match (index(shape, "shape["), index(para, "p[")) {
            (Some(shape), Some(para)) => Target::Paragraph { slide, shape, para },
            _ => Target::Other,
        },
        _ => Target::Other,
    })
}

fn locate<'s>(
    slides: &'s [Slide],
    node: &DocumentNodeId,
    op: &'static str,
) -> Result<(&'s Slide, &'s [Paragraph], usize), CodecError> {
    let Target::Paragraph { slide, shape, para } = target(node)? else {
        return Err(CodecError::UnsupportedOp {
            op,
            format: DocumentFormat::Pptx,
        });
    };
    let slide = slides
        .get(slide - 1)
        .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
    match slide.shapes.get(shape).map(|shape| &shape.content) {
        Some(Content::Text(paras)) if para < paras.len() => Ok((slide, paras.as_slice(), para)),
        _ => Err(CodecError::UnknownNode { node: node.clone() }),
    }
}

pub(super) fn paragraph_xml(para: &Paragraph, ppr: Option<&str>, text: &str) -> String {
    let mut out = String::from("<a:p>");
    out.push_str(ppr.unwrap_or_default());
    if text.is_empty() {
        out.push_str(para.end_rpr.as_deref().unwrap_or("<a:endParaRPr/>"));
        out.push_str("</a:p>");
        return out;
    }
    let rpr = para.rpr.as_deref().unwrap_or_default();
    for (line, segment) in text.split('\n').enumerate() {
        if line > 0 {
            out.push_str(&if rpr.is_empty() {
                "<a:br/>".to_owned()
            } else {
                format!("<a:br>{rpr}</a:br>")
            });
        }
        if !segment.is_empty() {
            out.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", escape(segment)));
        }
    }
    out.push_str(para.end_rpr.as_deref().unwrap_or_default());
    out.push_str("</a:p>");
    out
}

fn without_bullets(ppr: Option<&str>) -> Result<String, CodecError> {
    let Some(raw) = ppr else {
        return Ok("<a:pPr><a:buNone/></a:pPr>".to_owned());
    };
    let root = parse_xml("a:pPr", raw)?;
    if root.inner_start == root.end {
        let open = raw.trim_end().trim_end_matches("/>");
        return Ok(format!("{open}><a:buNone/></a:pPr>"));
    }
    let bullets = ["a:buNone", "a:buChar", "a:buAutoNum", "a:buBlip"];
    let mut edits: Vec<(usize, usize, String)> = root
        .children
        .iter()
        .filter(|child| bullets.contains(&child.name.as_str()))
        .map(|child| (child.start, child.end, String::new()))
        .collect();
    let at = edits.first().map(|edit| edit.0).unwrap_or_else(|| {
        root.children
            .iter()
            .find(|child| matches!(child.name.as_str(), "a:tabLst" | "a:defRPr" | "a:extLst"))
            .map_or(root.inner_end, |child| child.start)
    });
    edits.push((at, at, "<a:buNone/>".to_owned()));
    Ok(splice(raw, edits))
}

fn edit_slide(package: &mut Package<'_>, slide: &Slide, edit: (usize, usize, String)) {
    package.write(&slide.part, splice(&slide.xml, vec![edit]));
}

fn replace_text(
    package: &mut Package<'_>,
    node: &DocumentNodeId,
    text: &str,
) -> Result<(), CodecError> {
    let slides = projection::load(package)?;
    let (slide, paras, at) = locate(&slides, node, "replace_text")?;
    let para = &paras[at];
    let xml = paragraph_xml(para, para.ppr.as_deref(), text);
    edit_slide(package, slide, (para.start, para.end, xml));
    Ok(())
}

fn insert_after(
    package: &mut Package<'_>,
    node: &DocumentNodeId,
    kind: NodeKind,
    text: &str,
) -> Result<(), CodecError> {
    let slides = projection::load(package)?;
    let (slide, paras, at) = locate(&slides, node, "insert_after")?;
    let para = &paras[at];
    let ppr = match kind {
        NodeKind::ListItem => para.ppr.clone(),
        _ => Some(without_bullets(para.ppr.as_deref())?),
    };
    let xml = paragraph_xml(para, ppr.as_deref(), text);
    edit_slide(package, slide, (para.end, para.end, xml));
    Ok(())
}

fn delete(package: &mut Package<'_>, node: &DocumentNodeId) -> Result<(), CodecError> {
    if let Target::Slide(index) = target(node)? {
        return slides::delete_slide(package, index);
    }
    let slides = projection::load(package)?;
    let (slide, paras, at) = locate(&slides, node, "delete")?;
    let para = &paras[at];
    let replacement = if paras.len() > 1 {
        String::new()
    } else {
        paragraph_xml(para, para.ppr.as_deref(), "")
    };
    edit_slide(package, slide, (para.start, para.end, replacement));
    Ok(())
}

fn str_replace(package: &mut Package<'_>, old: &str, new: &str) -> Result<(), CodecError> {
    if old.is_empty() {
        return Err(CodecError::NoMatch);
    }
    let slides = projection::load(package)?;
    let mut hits = Vec::new();
    for (s, slide) in slides.iter().enumerate() {
        for (h, shape) in slide.shapes.iter().enumerate() {
            if let Content::Text(paras) = &shape.content {
                for (p, para) in paras.iter().enumerate() {
                    for _ in para.text.matches(old) {
                        hits.push((s, h, p, para.text.replacen(old, new, 1)));
                    }
                }
            }
        }
    }
    match hits.as_slice() {
        [] => Err(CodecError::NoMatch),
        [(s, h, p, text)] => {
            let node = DocumentNodeId::new(format!("slide[{}]/shape[{h}]/p[{p}]", s + 1));
            replace_text(package, &node, text)
        },
        many => Err(CodecError::AmbiguousMatch {
            matches: many.len(),
        }),
    }
}

fn verify(current: &Projection, ops: &[EditOp]) -> Result<(), CodecError> {
    for op in ops {
        let (node, expected) = match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                ..
            }
            | EditOp::Delete {
                node,
                expected_hash,
            } => (node, Some(expected_hash)),
            EditOp::InsertAfter { node, .. } => (node, None),
            EditOp::InsertSlide {
                after: Some(node), ..
            } => (node, None),
            _ => continue,
        };
        let found = current
            .find(node)
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
        if let Some(expected) = expected.filter(|expected| **expected != found.hash8) {
            return Err(CodecError::StaleAnchor {
                node: node.clone(),
                expected: expected.clone(),
                actual: found.hash8.clone(),
                current_text: found.text.clone().unwrap_or_default(),
            });
        }
    }
    Ok(())
}

fn apply_one(package: &mut Package<'_>, op: &EditOp) -> Result<(), CodecError> {
    match op {
        EditOp::ReplaceText { node, text, .. } => replace_text(package, node, text),
        EditOp::InsertAfter { node, kind, text } => insert_after(package, node, *kind, text),
        EditOp::Delete { node, .. } => delete(package, node),
        EditOp::StrReplace { old_str, new_str } => str_replace(package, old_str, new_str),
        EditOp::InsertSlide {
            after,
            layout,
            title,
            body,
        } => {
            let after = match after {
                Some(node) => match target(node)? {
                    Target::Slide(index) => Some(index),
                    _ => return Err(CodecError::UnknownNode { node: node.clone() }),
                },
                None => None,
            };
            let new = NewSlide {
                after,
                layout: layout.as_deref(),
                title: title.as_deref(),
                body,
            };
            slides::insert_slide(package, &new)
        },
        EditOp::SetCells { .. } | EditOp::SetHtml { .. } | EditOp::SetText { .. } => {
            Err(CodecError::UnsupportedOp {
                op: op.name(),
                format: DocumentFormat::Pptx,
            })
        },
    }
}

pub(super) fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(bytes)?;
    let current = Projection {
        format: DocumentFormat::Pptx,
        root: nodes::nodes(&projection::load(&mut package)?),
        content_sha256: content_hash(bytes),
    };
    verify(&current, ops)?;
    for op in ops {
        apply_one(&mut package, op)?;
    }
    package.finish()
}
