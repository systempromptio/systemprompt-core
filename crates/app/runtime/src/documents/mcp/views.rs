//! The folded outline, single-node paging, in-document search and the
//! changes since a revision, each rendered as hashlines.
//!
//! The outline shows headings, slides, sheets and tables, and list items
//! folded to the first three of each run followed by `gap blocks=N`. A node
//! read shows that node and its descendants. A search shows each innermost
//! block whose text contains the query, ignoring case. A changes read shows
//! the blocks that are new or changed since a revision, `gap blocks=N` for
//! each run of unchanged blocks, and `gone <id>` for each block deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::nodes::{ChangeItem, NodeChanges, changes};
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};

use super::lines::{is_outline, shows, structured, structured_line, unknown_node};

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
    revision: &DocumentRevisionId,
) -> Result<Vec<String>, McpError> {
    let node = node.ok_or_else(|| {
        McpError::invalid_params(
            "view node needs node_id; read the outline for node ids",
            None,
        )
    })?;
    let found = projection
        .find(node)
        .ok_or_else(|| unknown_node(node, revision))?;
    let subtree = Projection {
        format: projection.format,
        root: vec![found.clone()],
        content_sha256: projection.content_sha256.clone(),
    };
    Ok(structured(&subtree, |n| shows(n) || n.id == *node))
}

pub fn search_lines(projection: &Projection, text: Option<&str>) -> Result<Vec<String>, McpError> {
    let needle = text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| McpError::invalid_params("view search needs text to find", None))?
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
