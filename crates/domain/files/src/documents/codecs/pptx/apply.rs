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

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind,
};

use crate::documents::codecs::pending::text_marker;

use super::layout::NewSlide;
use super::paragraph::{markdown_xml, paragraph_xml, without_bullets};
use super::projection::{self, Content, Paragraph, Slide};
use super::slides;
use crate::documents::codecs::xml_tree::splice;
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

pub(super) fn locate<'s>(
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

pub(super) fn edit_slide(package: &mut Package<'_>, slide: &Slide, edit: (usize, usize, String)) {
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
) -> Result<usize, CodecError> {
    let slides = projection::load(package)?;
    let (slide, paras, at) = locate(&slides, node, "insert_after")?;
    let para = &paras[at];
    let ppr = match kind {
        NodeKind::ListItem => para.ppr.clone(),
        _ => Some(without_bullets(para.ppr.as_deref())?),
    };
    let xml = if kind == NodeKind::Pending {
        paragraph_xml(para, ppr.as_deref(), &text_marker(text))
    } else {
        paragraph_xml(para, ppr.as_deref(), text)
    };
    edit_slide(package, slide, (para.end, para.end, xml));
    Ok(1)
}

fn insert_markdown(
    package: &mut Package<'_>,
    node: Option<&DocumentNodeId>,
    markdown: &str,
) -> Result<usize, CodecError> {
    let slides = projection::load(package)?;
    let last = || {
        slides.iter().enumerate().rev().find_map(|(s, slide)| {
            slide
                .shapes
                .iter()
                .enumerate()
                .rev()
                .find_map(|(h, shape)| match &shape.content {
                    Content::Text(paras) if !paras.is_empty() => Some(DocumentNodeId::new(
                        format!("slide[{}]/shape[{h}]/p[{}]", s + 1, paras.len() - 1),
                    )),
                    _ => None,
                })
        })
    };
    let node = match node {
        Some(node) => node.clone(),
        None => last().ok_or(CodecError::UnsupportedOp {
            op: "append_markdown",
            format: DocumentFormat::Pptx,
        })?,
    };
    let (slide, paras, at) = locate(&slides, &node, "insert_after")?;
    let para = &paras[at];
    let xml = markdown_xml(para, markdown)?;
    let inserted = xml.matches("<a:p>").count();
    edit_slide(package, slide, (para.end, para.end, xml));
    Ok(inserted)
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Applied {
    pub removed: bool,
    pub inserted: usize,
}

pub(super) fn apply_one(package: &mut Package<'_>, op: &EditOp) -> Result<Applied, CodecError> {
    match op {
        EditOp::Delete { node, .. } => delete(package, node.resolved()?).map(|removed| Applied {
            removed,
            inserted: 0,
        }),
        other => apply_edit(package, other).map(|inserted| Applied {
            removed: false,
            inserted,
        }),
    }
}

fn apply_edit(package: &mut Package<'_>, op: &EditOp) -> Result<usize, CodecError> {
    match op {
        EditOp::ReplaceText { node, text, .. } => {
            replace_text(package, node.resolved()?, text).map(|()| 0)
        },
        EditOp::InsertAfter {
            node,
            kind,
            text,
            content_as: ContentAs::Text,
        } => insert_after(package, node.resolved()?, *kind, text),
        EditOp::InsertAfter { node, text, .. } => {
            insert_markdown(package, Some(node.resolved()?), text)
        },
        EditOp::AppendMarkdown { markdown } => insert_markdown(package, None, markdown),
        EditOp::StrReplace { old_str, new_str } => {
            str_replace(package, old_str, new_str).map(|()| 0)
        },
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
            slides::insert_slide(package, &new).map(|()| 0)
        },
        EditOp::Delete { .. }
        | EditOp::SetCells { .. }
        | EditOp::SetHtml { .. }
        | EditOp::SetText { .. }
        | EditOp::Restore { .. }
        | EditOp::InsertFromRevision { .. }
        | EditOp::SetMarks(_)
        | EditOp::SetBlockType(_)
        | EditOp::InsertTable(_)
        | EditOp::TableInsertRow(_)
        | EditOp::TableInsertCol(_)
        | EditOp::TableDelete(_)
        | EditOp::InsertImage(_)

        | EditOp::SetParagraph(_)

        | EditOp::InsertBlock(_)

        | EditOp::UpdateTable(_)

        | EditOp::SetSection(_)

        | EditOp::SetStyle(_)

        | EditOp::ApplyTheme(_) => Err(CodecError::UnsupportedOp {
            op: op.name(),
            format: DocumentFormat::Pptx,
        }),
    }
}
