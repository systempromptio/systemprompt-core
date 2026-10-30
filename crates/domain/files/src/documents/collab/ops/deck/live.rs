//! The `collab-loro` translation of a deck op batch into a Loro update,
//! with the `slide[N]/shape[S]/p[P]` (or `slide[N]` for an inserted slide)
//! ids it wrote, positional as of the op that wrote each.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, LoroDoc, LoroMap, LoroMovableList, LoroText, PeerID};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::EditOp;

use super::slide::{frame_for, new_block, text_shape};
use crate::documents::engine::loro::deck::{
    DeckSlide, ShapeKind, insert_paragraph, insert_slide, slides,
};
use crate::documents::engine::loro::rich::{WriteError, block_map, text_of};

#[derive(Debug, thiserror::Error)]
pub(crate) enum DeckOpError {
    #[error("op {op} is not translated for a live deck")]
    Unsupported { op: &'static str },
    #[error("node {node} names nothing in the live deck")]
    UnknownNode { node: String },
    #[error("old_str matches {matches} places")]
    Ambiguous { matches: usize },
    #[error("old_str matches nothing")]
    NoMatch,
    #[error(transparent)]
    Write(WriteError),
}

impl From<loro::LoroError> for DeckOpError {
    fn from(error: loro::LoroError) -> Self {
        Self::Write(Box::new(error))
    }
}

fn unknown(id: &str) -> DeckOpError {
    DeckOpError::UnknownNode {
        node: id.to_owned(),
    }
}

enum Node {
    Slide(usize),
    Paragraph(usize, usize, usize),
}

fn index(segment: &str, prefix: &str) -> Option<usize> {
    segment
        .strip_prefix(prefix)?
        .strip_suffix(']')?
        .parse()
        .ok()
}

fn node(id: &str) -> Result<Node, DeckOpError> {
    let parts: Vec<&str> = id.split('/').collect();
    let slide = parts
        .first()
        .and_then(|s| index(s, "slide["))
        .filter(|n| *n > 0)
        .ok_or_else(|| unknown(id))?
        - 1;
    match parts.as_slice() {
        [_] => Ok(Node::Slide(slide)),
        [_, shape, para] => match (index(shape, "shape["), index(para, "p[")) {
            (Some(shape), Some(para)) => Ok(Node::Paragraph(slide, shape, para)),
            _ => Err(unknown(id)),
        },
        _ => Err(unknown(id)),
    }
}

fn child_list(map: &LoroMap, key: &str) -> Option<LoroMovableList> {
    match map.get(key)? {
        loro::ValueOrContainer::Container(loro::Container::MovableList(list)) => Some(list),
        _ => None,
    }
}

fn paragraphs(doc: &LoroDoc, slide: usize, shape: usize) -> Option<LoroMovableList> {
    let slide = block_map(&slides(doc), slide)?;
    let shape = block_map(&child_list(&slide, "shapes")?, shape)?;
    child_list(&shape, "paragraphs")
}

fn paragraph_text(
    doc: &LoroDoc,
    id: &str,
) -> Result<(LoroMovableList, usize, LoroText), DeckOpError> {
    let Node::Paragraph(slide, shape, para) = node(id)? else {
        return Err(unknown(id));
    };
    let list = paragraphs(doc, slide, shape).ok_or_else(|| unknown(id))?;
    let text = block_map(&list, para)
        .and_then(|map| text_of(&map))
        .ok_or_else(|| unknown(id))?;
    Ok((list, para, text))
}

fn para_id(slide: usize, shape: usize, para: usize) -> String {
    format!("slide[{}]/shape[{shape}]/p[{para}]", slide + 1)
}

fn str_replace(doc: &LoroDoc, old: &str, new: &str) -> Result<Vec<String>, DeckOpError> {
    if old.is_empty() {
        return Err(DeckOpError::NoMatch);
    }
    let mut hits = Vec::new();
    let list = slides(doc);
    for s in 0..list.len() {
        let Some(shapes) = block_map(&list, s).and_then(|m| child_list(&m, "shapes")) else {
            continue;
        };
        for h in 0..shapes.len() {
            let Some(paras) = paragraphs(doc, s, h) else {
                continue;
            };
            for p in 0..paras.len() {
                let Some(text) = block_map(&paras, p).and_then(|m| text_of(&m)) else {
                    continue;
                };
                let held = text.to_string();
                hits.extend(
                    held.match_indices(old).map(|(at, _)| {
                        (text.clone(), held[..at].chars().count(), para_id(s, h, p))
                    }),
                );
            }
        }
    }
    match hits.as_slice() {
        [] => Err(DeckOpError::NoMatch),
        [(text, at, id)] => {
            text.splice(*at, old.chars().count(), new)?;
            Ok(vec![id.clone()])
        },
        many => Err(DeckOpError::Ambiguous {
            matches: many.len(),
        }),
    }
}

fn apply(doc: &LoroDoc, op: &EditOp) -> Result<Vec<String>, DeckOpError> {
    let resolved = |target: &systemprompt_models::documents::NodeTarget| {
        target
            .id()
            .map(|id| id.as_str().to_owned())
            .ok_or(DeckOpError::Unsupported { op: op.name() })
    };
    match op {
        EditOp::ReplaceText { node, text, .. } => {
            let id = resolved(node)?;
            let (_, _, held) = paragraph_text(doc, &id)?;
            held.update(text, loro::UpdateOptions::default())
                .map_err(|e| DeckOpError::Write(Box::new(e)))?;
            Ok(vec![id])
        },
        EditOp::StrReplace { old_str, new_str } => str_replace(doc, old_str, new_str),
        EditOp::InsertAfter {
            node, kind, text, ..
        } => {
            let id = resolved(node)?;
            let Node::Paragraph(slide, shape, _) = self::node(&id)? else {
                return Err(DeckOpError::UnknownNode { node: id });
            };
            let (list, at, _) = paragraph_text(doc, &id)?;
            let mut written = Vec::new();
            for (offset, line) in text.split('\n').enumerate() {
                insert_paragraph(
                    &list,
                    at + 1 + offset,
                    &new_block(line, super::is_list_item(*kind)),
                )
                .map_err(DeckOpError::Write)?;
                written.push(para_id(slide, shape, at + 1 + offset));
            }
            Ok(written)
        },
        EditOp::Delete { node, .. } => match self::node(&resolved(node)?)? {
            Node::Slide(at) if at < slides(doc).len() => {
                slides(doc).delete(at, 1)?;
                Ok(Vec::new())
            },
            Node::Paragraph(..) => {
                let (list, at, _) = paragraph_text(doc, &resolved(node)?)?;
                list.delete(at, 1)?;
                Ok(Vec::new())
            },
            Node::Slide(_) => Err(DeckOpError::UnknownNode {
                node: resolved(node)?,
            }),
        },
        EditOp::InsertSlide {
            after,
            layout,
            title,
            body,
        } => new_slide(doc, after.as_ref(), layout.clone(), title.as_ref(), body),
        other => Err(DeckOpError::Unsupported { op: other.name() }),
    }
}

fn new_slide(
    doc: &LoroDoc,
    after: Option<&DocumentNodeId>,
    layout: Option<String>,
    title: Option<&String>,
    body: &[String],
) -> Result<Vec<String>, DeckOpError> {
    let list = slides(doc);
    let at = match after {
        Some(id) => match node(id.as_str())? {
            Node::Slide(at) if at < list.len() => at + 1,
            _ => {
                return Err(DeckOpError::UnknownNode {
                    node: id.as_str().to_owned(),
                });
            },
        },
        None => list.len(),
    };
    let mut shapes = Vec::new();
    if let Some(title) = title {
        shapes.push(text_shape(
            ShapeKind::Title,
            frame_for(doc, ShapeKind::Title),
            std::slice::from_ref(title),
            false,
        ));
    }
    if !body.is_empty() {
        let frame = frame_for(doc, ShapeKind::Body);
        shapes.push(text_shape(ShapeKind::Body, frame, body, true));
    }
    let slide = DeckSlide {
        id: uuid::Uuid::new_v4().to_string(),
        layout,
        shapes,
    };
    let mut written = vec![format!("slide[{}]", at + 1)];
    for (h, shape) in slide.shapes.iter().enumerate() {
        written.extend((0..shape.paragraphs.len()).map(|p| para_id(at, h, p)));
    }
    insert_slide(&list, at, &slide).map_err(DeckOpError::Write)?;
    Ok(written)
}

type DeckUpdate = Option<(Vec<u8>, Vec<String>)>;

pub(crate) fn deck_update(
    doc: &LoroDoc,
    ops: &[EditOp],
    peer: PeerID,
) -> Result<DeckUpdate, DeckOpError> {
    let before = doc.oplog_vv();
    let draft = doc.fork();
    crate::documents::engine::loro::rich::configure(&draft);
    draft.set_peer_id(peer)?;
    let mut touched: Vec<String> = Vec::new();
    for op in ops {
        for id in apply(&draft, op)? {
            if !touched.contains(&id) {
                touched.push(id);
            }
        }
    }
    draft.set_next_commit_origin("agent");
    draft.commit();
    if draft.oplog_vv() == before {
        return Ok(None);
    }
    draft
        .export(ExportMode::updates(&before))
        .map(|update| Some((update, touched)))
        .map_err(|e| DeckOpError::Write(Box::new(e)))
}
