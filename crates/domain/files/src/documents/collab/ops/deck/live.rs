//! The `collab-loro` translation of a deck op batch into a Loro update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, LoroDoc, LoroMap, LoroMovableList, LoroText, PeerID};
use systemprompt_models::documents::EditOp;

use crate::documents::engine::loro::deck::{
    DeckShape, DeckSlide, ShapeKind, insert_paragraph, insert_slide, slides,
};
use crate::documents::engine::loro::rich::{WriteError, block_map, text_of};
use crate::documents::model::layout::ParaAttrs;
use crate::documents::model::{Block, BlockKind};

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
    let unknown = || DeckOpError::UnknownNode {
        node: id.to_owned(),
    };
    let parts: Vec<&str> = id.split('/').collect();
    let slide = parts
        .first()
        .and_then(|s| index(s, "slide["))
        .filter(|n| *n > 0)
        .ok_or_else(unknown)?
        - 1;
    match parts.as_slice() {
        [_] => Ok(Node::Slide(slide)),
        [_, shape, para] => match (index(shape, "shape["), index(para, "p[")) {
            (Some(shape), Some(para)) => Ok(Node::Paragraph(slide, shape, para)),
            _ => Err(unknown()),
        },
        _ => Err(unknown()),
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
        return Err(DeckOpError::UnknownNode {
            node: id.to_owned(),
        });
    };
    let list = paragraphs(doc, slide, shape).ok_or_else(|| DeckOpError::UnknownNode {
        node: id.to_owned(),
    })?;
    let text = block_map(&list, para)
        .and_then(|map| text_of(&map))
        .ok_or_else(|| DeckOpError::UnknownNode {
            node: id.to_owned(),
        })?;
    Ok((list, para, text))
}

fn new_block(text: &str, list_item: bool) -> Block {
    Block {
        id: uuid::Uuid::new_v4().to_string(),
        kind: if list_item {
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
        text: text.to_owned(),
        marks: Vec::new(),
    }
}

fn text_shape(kind: ShapeKind, lines: &[String], list_item: bool) -> DeckShape {
    DeckShape {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        frame: None,
        src: None,
        paragraphs: lines
            .iter()
            .map(|line| new_block(line, list_item))
            .collect(),
    }
}

fn str_replace(doc: &LoroDoc, old: &str, new: &str) -> Result<(), DeckOpError> {
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
                    held.match_indices(old)
                        .map(|(at, _)| (text.clone(), held[..at].chars().count())),
                );
            }
        }
    }
    match hits.as_slice() {
        [] => Err(DeckOpError::NoMatch),
        [(text, at)] => {
            text.splice(*at, old.chars().count(), new)?;
            Ok(())
        },
        many => Err(DeckOpError::Ambiguous {
            matches: many.len(),
        }),
    }
}

fn apply(doc: &LoroDoc, op: &EditOp) -> Result<(), DeckOpError> {
    let resolved = |target: &systemprompt_models::documents::NodeTarget| {
        target
            .id()
            .map(|id| id.as_str().to_owned())
            .ok_or(DeckOpError::Unsupported { op: op.name() })
    };
    match op {
        EditOp::ReplaceText { node, text, .. } => {
            let (_, _, held) = paragraph_text(doc, &resolved(node)?)?;
            held.update(text, loro::UpdateOptions::default())
                .map_err(|e| DeckOpError::Write(Box::new(e)))
        },
        EditOp::StrReplace { old_str, new_str } => str_replace(doc, old_str, new_str),
        EditOp::InsertAfter {
            node, kind, text, ..
        } => {
            let (list, at, _) = paragraph_text(doc, &resolved(node)?)?;
            for (offset, line) in text.split('\n').enumerate() {
                insert_paragraph(
                    &list,
                    at + 1 + offset,
                    &new_block(line, super::is_list_item(*kind)),
                )
                .map_err(DeckOpError::Write)?;
            }
            Ok(())
        },
        EditOp::Delete { node, .. } => match self::node(&resolved(node)?)? {
            Node::Slide(at) if at < slides(doc).len() => Ok(slides(doc).delete(at, 1)?),
            Node::Paragraph(..) => {
                let (list, at, _) = paragraph_text(doc, &resolved(node)?)?;
                Ok(list.delete(at, 1)?)
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
        } => {
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
                    std::slice::from_ref(title),
                    false,
                ));
            }
            if !body.is_empty() {
                shapes.push(text_shape(ShapeKind::Body, body, true));
            }
            let slide = DeckSlide {
                id: uuid::Uuid::new_v4().to_string(),
                layout: layout.clone(),
                shapes,
            };
            insert_slide(&list, at, &slide).map_err(DeckOpError::Write)
        },
        other => Err(DeckOpError::Unsupported { op: other.name() }),
    }
}

pub(crate) fn deck_update(
    doc: &LoroDoc,
    ops: &[EditOp],
    peer: PeerID,
) -> Result<Option<Vec<u8>>, DeckOpError> {
    let before = doc.oplog_vv();
    let draft = doc.fork();
    crate::documents::engine::loro::rich::configure(&draft);
    draft.set_peer_id(peer)?;
    for op in ops {
        apply(&draft, op)?;
    }
    draft.set_next_commit_origin("agent");
    draft.commit();
    if draft.oplog_vv() == before {
        return Ok(None);
    }
    draft
        .export(ExportMode::updates(&before))
        .map(Some)
        .map_err(|e| DeckOpError::Write(Box::new(e)))
}
