//! Applies edit operations to a deck by splicing the touched paragraphs of a
//! slide part, or by rewriting the presentation parts for a slide insert or
//! removal.
//!
//! Every anchored operation is checked against the deck as read before any
//! operation runs, so a batch applies all or none. Node ids in every
//! operation name the deck as read; the rebase module moves them past the
//! inserts and removals of earlier operations in the batch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, DocumentFormat, EditOp, NodeKind};

use super::layout::NewSlide;
use super::projection::{self, Content, Paragraph, Slide};
use super::slides;
use crate::documents::codecs::xml_tree::{parse_xml, splice};
use crate::documents::codecs::zip_package::Package;

pub(super) enum Target {
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

pub(super) fn target(node: &DocumentNodeId) -> Result<Target, CodecError> {
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
    let at = edits.first().map_or_else(
        || {
            root.children
                .iter()
                .find(|child| matches!(child.name.as_str(), "a:tabLst" | "a:defRPr" | "a:extLst"))
                .map_or(root.inner_end, |child| child.start)
        },
        |edit| edit.0,
    );
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

fn delete(package: &mut Package<'_>, node: &DocumentNodeId) -> Result<bool, CodecError> {
    if let Target::Slide(index) = target(node)? {
        return slides::delete_slide(package, index).map(|()| true);
    }
    let slides = projection::load(package)?;
    let (slide, paras, at) = locate(&slides, node, "delete")?;
    let para = &paras[at];
    let removed = paras.len() > 1;
    let replacement = if removed {
        String::new()
    } else {
        paragraph_xml(para, para.ppr.as_deref(), "")
    };
    edit_slide(package, slide, (para.start, para.end, replacement));
    Ok(removed)
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

pub(super) fn apply_one(package: &mut Package<'_>, op: &EditOp) -> Result<bool, CodecError> {
    match op {
        EditOp::Delete { node, .. } => delete(package, node),
        other => apply_edit(package, other).map(|()| false),
    }
}

fn apply_edit(package: &mut Package<'_>, op: &EditOp) -> Result<(), CodecError> {
    match op {
        EditOp::ReplaceText { node, text, .. } => replace_text(package, node, text),
        EditOp::InsertAfter { node, kind, text } => insert_after(package, node, *kind, text),
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
        EditOp::Delete { .. }
        | EditOp::SetCells { .. }
        | EditOp::SetHtml { .. }
        | EditOp::SetText { .. } => Err(CodecError::UnsupportedOp {
            op: op.name(),
            format: DocumentFormat::Pptx,
        }),
    }
}
