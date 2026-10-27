//! Projection of the slide model into hash-anchored nodes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{NodeKind, ProjectionNode, hash8};

use super::projection::{Content, Shape, Slide};

fn node(
    id: String,
    kind: NodeKind,
    text: Option<String>,
    children: Vec<ProjectionNode>,
) -> ProjectionNode {
    let hash = match &text {
        Some(text) if children.is_empty() => hash8(text),
        _ => hash8(
            &children
                .iter()
                .map(|child| child.hash8.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ),
    };
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text,
        hash8: hash,
        children,
        attrs: BTreeMap::new(),
    }
}

fn paragraph_nodes(
    prefix: &str,
    lines: impl Iterator<Item = (bool, String)>,
) -> Vec<ProjectionNode> {
    lines
        .enumerate()
        .map(|(index, (bullet, text))| {
            let mut para = node(
                format!("{prefix}/p[{index}]"),
                NodeKind::Paragraph,
                Some(text),
                Vec::new(),
            );
            if bullet {
                para.attrs.insert("bullet".to_owned(), "true".to_owned());
            }
            para
        })
        .collect()
}

fn shape_node(prefix: &str, shape: &Shape) -> ProjectionNode {
    let (kind, text, children) = match &shape.content {
        Content::Text(paras) => (
            NodeKind::Shape,
            None,
            paragraph_nodes(
                prefix,
                paras.iter().map(|para| (para.bullet, para.text.clone())),
            ),
        ),
        Content::Table(rows) => {
            let rows = rows
                .iter()
                .enumerate()
                .map(|(r, cells)| {
                    let cells = cells
                        .iter()
                        .enumerate()
                        .map(|(c, text)| {
                            node(
                                format!("{prefix}/tbl/r[{r}]/c[{c}]"),
                                NodeKind::Cell,
                                Some(text.clone()),
                                Vec::new(),
                            )
                        })
                        .collect();
                    node(format!("{prefix}/tbl/r[{r}]"), NodeKind::Row, None, cells)
                })
                .collect();
            (
                NodeKind::Shape,
                None,
                vec![node(format!("{prefix}/tbl"), NodeKind::Table, None, rows)],
            )
        },
        Content::Image(_) => (NodeKind::Image, Some(String::new()), Vec::new()),
        Content::Other => (NodeKind::Shape, Some(String::new()), Vec::new()),
    };
    let mut shape_node = node(prefix.to_owned(), kind, text, children);
    if let Content::Image(rel_id) = &shape.content {
        shape_node.hash8 = hash8(rel_id);
        shape_node.attrs.insert("rel_id".to_owned(), rel_id.clone());
    }
    for (key, value) in [("name", &shape.name), ("placeholder", &shape.placeholder)] {
        if let Some(value) = value {
            shape_node.attrs.insert(key.to_owned(), value.clone());
        }
    }
    shape_node
}

pub(super) fn nodes(slides: &[Slide]) -> Vec<ProjectionNode> {
    slides
        .iter()
        .enumerate()
        .map(|(index, slide)| {
            let id = format!("slide[{}]", index + 1);
            let mut children: Vec<ProjectionNode> = slide
                .shapes
                .iter()
                .enumerate()
                .map(|(s, shape)| shape_node(&format!("{id}/shape[{s}]"), shape))
                .collect();
            let mut slide_node = node(id.clone(), NodeKind::Slide, None, children.clone());
            if !slide.notes.is_empty() {
                let notes_id = format!("{id}/notes");
                let paras = paragraph_nodes(
                    &notes_id,
                    slide.notes.iter().map(|text| (false, text.clone())),
                );
                children.push(node(notes_id, NodeKind::Body, None, paras));
            }
            slide_node.children = children;
            slide_node.text = Some(slide.title().unwrap_or_default());
            slide_node
                .attrs
                .insert("part".to_owned(), slide.part.clone());
            if let Some(layout) = &slide.layout {
                slide_node.attrs.insert("layout".to_owned(), layout.clone());
            }
            slide_node
        })
        .collect()
}
