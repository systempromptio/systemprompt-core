//! Resolution of quote targets against the projection a batch was read
//! from.
//!
//! [`resolve_targets`] runs once per batch, before any op applies, so every
//! quote in the batch matches the document as it stood before the call: an
//! op that inserts a block cannot shift what a later op's quote finds. A
//! resolved op carries the matched node's id and, unless the caller quoted
//! one, its current hash. A `replace_text` with Markdown content resolves to
//! an `insert_after` of the lowered blocks followed by a `delete` of the
//! node, so codecs only lower Markdown on insert.
//!
//! A block is a node with text, outside any subtree marked `read_only`,
//! that has no descendant which also matches; a quote that spans two
//! paragraphs therefore matches their container.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::DocumentNodeId;

use super::edit_op::{ContentAs, EditOp, NodeTarget, QuoteTarget};
use super::projection::{Projection, ProjectionNode};

pub const READ_ONLY_ATTR: &str = "read_only";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosestBlock {
    pub node: DocumentNodeId,
    pub hash: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("no block contains \"{find}\"")]
    FindNone {
        find: String,
        closest: Option<ClosestBlock>,
    },
    #[error("node {node} does not exist in this document")]
    UnknownNode { node: DocumentNodeId },
    #[error("expected_hash is required when node names a node id")]
    MissingHash { node: DocumentNodeId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedBatch {
    pub ops: Vec<EditOp>,
    pub matches: Vec<usize>,
}

struct Resolved {
    node: DocumentNodeId,
    hash: String,
    matches: Option<usize>,
}

pub fn resolve_targets(
    projection: &Projection,
    ops: Vec<EditOp>,
) -> Result<ResolvedBatch, TargetError> {
    let mut batch = ResolvedBatch {
        ops: Vec::with_capacity(ops.len()),
        matches: Vec::new(),
    };
    for op in ops {
        match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                content_as,
            } => {
                let resolved = resolve(projection, &node, expected_hash, true)?;
                batch.matches.extend(resolved.matches);
                if content_as == ContentAs::Markdown {
                    batch.ops.push(EditOp::InsertAfter {
                        node: NodeTarget::Id(resolved.node.clone()),
                        kind: super::projection::NodeKind::Paragraph,
                        text,
                        content_as,
                    });
                    batch.ops.push(EditOp::Delete {
                        node: NodeTarget::Id(resolved.node),
                        expected_hash: Some(resolved.hash),
                    });
                } else {
                    batch.ops.push(EditOp::ReplaceText {
                        node: NodeTarget::Id(resolved.node),
                        expected_hash: Some(resolved.hash),
                        text,
                        content_as,
                    });
                }
            },
            EditOp::Delete {
                node,
                expected_hash,
            } => {
                let resolved = resolve(projection, &node, expected_hash, true)?;
                batch.matches.extend(resolved.matches);
                batch.ops.push(EditOp::Delete {
                    node: NodeTarget::Id(resolved.node),
                    expected_hash: Some(resolved.hash),
                });
            },
            EditOp::InsertAfter {
                node,
                kind,
                text,
                content_as,
            } => {
                let resolved = resolve(projection, &node, None, false)?;
                batch.matches.extend(resolved.matches);
                batch.ops.push(EditOp::InsertAfter {
                    node: NodeTarget::Id(resolved.node),
                    kind,
                    text,
                    content_as,
                });
            },
            other => batch.ops.push(other),
        }
    }
    Ok(batch)
}

fn resolve(
    projection: &Projection,
    target: &NodeTarget,
    expected_hash: Option<String>,
    anchored: bool,
) -> Result<Resolved, TargetError> {
    match target {
        NodeTarget::Id(node) => {
            let hash = match expected_hash {
                Some(hash) => hash,
                None if anchored => return Err(TargetError::MissingHash { node: node.clone() }),
                None => String::new(),
            };
            Ok(Resolved {
                node: node.clone(),
                hash,
                matches: None,
            })
        },
        NodeTarget::Find(quote) => {
            let (hits, count) = find(projection, quote)?;
            let hit = hits.ok_or_else(|| TargetError::FindNone {
                find: quote.find.clone(),
                closest: closest(projection, quote),
            })?;
            Ok(Resolved {
                node: hit.id.clone(),
                hash: expected_hash.unwrap_or_else(|| hit.hash8.clone()),
                matches: Some(count),
            })
        },
    }
}

fn scope<'p>(
    projection: &'p Projection,
    within: Option<&DocumentNodeId>,
) -> Result<&'p [ProjectionNode], TargetError> {
    within.map_or(Ok(&projection.root), |id| {
        projection
            .find(id)
            .map(std::slice::from_ref)
            .ok_or_else(|| TargetError::UnknownNode { node: id.clone() })
    })
}

fn find<'p>(
    projection: &'p Projection,
    quote: &QuoteTarget,
) -> Result<(Option<&'p ProjectionNode>, usize), TargetError> {
    let mut hits = Vec::new();
    if !quote.find.is_empty() {
        collect(
            scope(projection, quote.within.as_ref())?,
            &quote.find,
            &mut hits,
        );
    }
    let nth = quote.nth.map_or(1, std::num::NonZeroUsize::get);
    Ok((hits.get(nth - 1).copied(), hits.len()))
}

fn collect<'p>(nodes: &'p [ProjectionNode], needle: &str, hits: &mut Vec<&'p ProjectionNode>) {
    for node in nodes {
        if is_read_only(node) {
            continue;
        }
        let before = hits.len();
        collect(&node.children, needle, hits);
        if hits.len() == before
            && node
                .text
                .as_deref()
                .is_some_and(|text| text.contains(needle))
        {
            hits.push(node);
        }
    }
}

fn is_read_only(node: &ProjectionNode) -> bool {
    node.attrs
        .get(READ_ONLY_ATTR)
        .is_some_and(|flag| flag == "true")
}

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn closest(projection: &Projection, quote: &QuoteTarget) -> Option<ClosestBlock> {
    let wanted = words(&quote.find);
    let nodes = scope(projection, quote.within.as_ref()).ok()?;
    let mut best: Option<(usize, &ProjectionNode)> = None;
    let mut stack: Vec<&ProjectionNode> = nodes.iter().rev().collect();
    while let Some(node) = stack.pop() {
        if is_read_only(node) {
            continue;
        }
        stack.extend(node.children.iter().rev());
        let Some(text) = node.text.as_deref() else {
            continue;
        };
        let score = words(text).intersection(&wanted).count();
        if score > best.map_or(0, |(top, _)| top) {
            best = Some((score, node));
        }
    }
    best.map(|(_, node)| ClosestBlock {
        node: node.id.clone(),
        hash: node.hash8.clone(),
        text: node.text.clone().unwrap_or_default(),
    })
}
