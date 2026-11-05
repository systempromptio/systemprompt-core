//! The folded outline, single-node paging, in-document search and the
//! changes since a revision, each rendered as hashlines.
//!
//! The outline shows headings, slides, sheets and tables, and list items
//! folded to the first three of each run followed by `gap blocks=N`. A node
//! read shows that node and its descendants. A search shows each innermost
//! block whose text contains the query, ignoring case. A changes read shows
//! the blocks that are new or changed since a revision, `gap blocks=N` for
//! each run of unchanged blocks, and `gone <id>` for each block deleted.
//! The text views leave out read-only roots unless the read is verbose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde::Serialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::nodes::{ChangeItem, NodeChanges, changes};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};
use systemprompt_provider_contracts::documents::READ_ONLY_ATTR;

use super::lines::{is_outline, shows, structured, structured_line};

const FOLD_AFTER: usize = 3;

fn gap(blocks: usize) -> String {
    format!("gap blocks={blocks}")
}

#[must_use]
pub fn outline(projection: &Projection) -> Vec<String> {
    let mut lines = Vec::new();
    let mut slide = 0;
    let mut run = 0;
    let mut folded = 0;
    for node in projection.nodes() {
        if node.kind == NodeKind::Slide {
            slide += 1;
        }
        if !shows(node) {
            continue;
        }
        if node.kind == NodeKind::ListItem {
            run += 1;
            if run <= FOLD_AFTER {
                lines.push(structured_line(node, slide));
            } else {
                folded += 1;
            }
            continue;
        }
        if folded > 0 {
            lines.push(gap(folded));
        }
        run = 0;
        folded = 0;
        if is_outline(node.kind) {
            lines.push(structured_line(node, slide));
        }
    }
    if folded > 0 {
        lines.push(gap(folded));
    }
    lines
}

pub fn node_lines(
    projection: &Projection,
    node: Option<&DocumentNodeId>,
) -> Result<Vec<String>, DocumentsError> {
    let node = node.ok_or(DocumentsError::InvalidParams {
        detail: "view node needs node_id; read the outline for node ids",
    })?;
    let found = projection
        .find(node)
        .ok_or_else(|| DocumentsError::UnknownNode { node: node.clone() })?;
    let subtree = Projection {
        format: projection.format,
        root: vec![found.clone()],
        content_sha256: projection.content_sha256.clone(),
    };
    Ok(structured(&subtree, |n| shows(n) || n.id == *node))
}

pub fn search_lines(
    projection: &Projection,
    text: Option<&str>,
) -> Result<Vec<String>, DocumentsError> {
    let needle = text
        .filter(|text| !text.trim().is_empty())
        .ok_or(DocumentsError::InvalidParams {
            detail: "view search needs text to find",
        })?
        .to_lowercase();
    let contains = |node: &ProjectionNode| {
        node.text
            .as_deref()
            .is_some_and(|text| text.to_lowercase().contains(&needle))
    };
    Ok(structured(projection, |node| {
        contains(node) && !node.children.iter().any(&contains)
    }))
}

#[must_use]
pub fn change_lines(head: &Projection, changes: &NodeChanges<'_>) -> Vec<String> {
    let mut slides = HashMap::new();
    let mut slide = 0;
    for node in head.nodes() {
        if node.kind == NodeKind::Slide {
            slide += 1;
        }
        slides.insert(&node.id, slide);
    }
    changes
        .items
        .iter()
        .map(|item| match item {
            ChangeItem::Node(node) => {
                structured_line(node, slides.get(&node.id).copied().unwrap_or_default())
            },
            ChangeItem::Gap(blocks) => gap(*blocks),
        })
        .chain(changes.gone.iter().map(|id| format!("gone {id}")))
        .collect()
}

#[must_use]
pub fn proposal_lines(base: &Projection, proposed: &Projection) -> Vec<String> {
    change_lines(proposed, &changes(base, proposed, shows))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalChangeKind {
    Added,
    Changed,
    Removed,
}

/// One block a proposal touches, in words a reviewer reads rather than the
/// hashlines an agent edits against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProposalChange {
    pub node: DocumentNodeId,
    pub kind: ProposalChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[must_use]
pub fn proposal_changes(base: &Projection, proposed: &Projection) -> Vec<ProposalChange> {
    let found = changes(base, proposed, shows);
    let text_in = |projection: &Projection, id: &DocumentNodeId| {
        projection.find(id).and_then(|node| node.text.clone())
    };
    let touched = found.items.iter().filter_map(|item| match item {
        ChangeItem::Node(node) => {
            let before = base.find(&node.id);
            Some(ProposalChange {
                node: node.id.clone(),
                kind: if before.is_some() {
                    ProposalChangeKind::Changed
                } else {
                    ProposalChangeKind::Added
                },
                before: before.and_then(|held| held.text.clone()),
                after: node.text.clone(),
            })
        },
        ChangeItem::Gap(_) => None,
    });
    let removed = found.gone.iter().map(|id| ProposalChange {
        node: id.clone(),
        kind: ProposalChangeKind::Removed,
        before: text_in(base, id),
        after: None,
    });
    touched.chain(removed).collect()
}

pub(super) fn shown_roots(projection: &Projection, verbose: bool) -> Option<Projection> {
    let read_only = |node: &ProjectionNode| {
        node.attrs
            .get(READ_ONLY_ATTR)
            .is_some_and(|flag| flag == "true")
    };
    (!verbose && projection.root.iter().any(read_only)).then(|| Projection {
        root: projection
            .root
            .iter()
            .filter(|node| !read_only(node))
            .cloned()
            .collect(),
        ..projection.clone()
    })
}
