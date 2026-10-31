//! The `written` lines of a live edit: the lines of the live projection
//! naming the ids a batch wrote, by node id or by path.
//!
//! A default read hides a document's read-only roots (a Markdown document's
//! parsed blocks, shown beside its source lines); a write that named a node
//! under one is shown by the visible line holding the same text instead. A
//! workbook cell the projection does not hold (past its cell cap) is shown
//! from the input the batch wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_files::documents::nodes::path_of;
use systemprompt_models::documents::{Projection, ProjectionNode};
use systemprompt_provider_contracts::documents::{READ_ONLY_ATTR, hash8};

use super::lines::{MAX_WRITTEN, folded_cell, shows, structured};
use super::outline::shown_roots;

#[must_use]
pub fn touched_lines(
    projection: &Projection,
    touched: &[String],
    values: &[(String, String)],
    verbose: bool,
) -> Vec<String> {
    let named = |node: &ProjectionNode| {
        touched
            .iter()
            .any(|id| id == node.id.as_str() || id == path_of(node))
    };
    let shown = shown_roots(projection, verbose);
    let mut mirrored: Vec<&str> = match &shown {
        Some(_) => hidden_roots(projection)
            .flat_map(|root| std::iter::once(root).chain(subtree(root)))
            .filter(|node| shows(node) && named(node))
            .map(|node| node.hash8.as_str())
            .collect(),
        None => Vec::new(),
    };
    let projection = shown.as_ref().unwrap_or(projection);
    let mut lines = structured(projection, |node| {
        if !shows(node) {
            return false;
        }
        if named(node) || (folded_cell(node) && node.children.iter().any(named)) {
            return true;
        }
        let mirror = mirrored
            .iter()
            .position(|hash| *hash == node.hash8.as_str());
        mirror.map(|at| mirrored.swap_remove(at)).is_some()
    });
    let projected: HashSet<&str> = projection
        .nodes()
        .flat_map(|node| [node.id.as_str(), path_of(node)])
        .collect();
    lines.extend(
        values
            .iter()
            .filter(|(id, _)| !projected.contains(id.as_str()))
            .map(|(id, value)| format!("{id}:{}| | {value}", hash8(value))),
    );
    lines.truncate(MAX_WRITTEN);
    lines
}

fn hidden_roots(projection: &Projection) -> impl Iterator<Item = &ProjectionNode> {
    projection.root.iter().filter(|node| {
        node.attrs
            .get(READ_ONLY_ATTR)
            .is_some_and(|flag| flag == "true")
    })
}

fn subtree(node: &ProjectionNode) -> Vec<&ProjectionNode> {
    let mut out = Vec::new();
    for child in &node.children {
        out.push(child);
        out.extend(subtree(child));
    }
    out
}
