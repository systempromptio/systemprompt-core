//! The id-addressed view of a projection and the translation of ops back to
//! codec paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{EditOp, NodeTarget, Projection, ProjectionNode};

use crate::documents::models::NodeIdEntry;

pub const PATH_ATTR: &str = "path";

#[must_use]
pub fn with_ids(mut projection: Projection, entries: &[NodeIdEntry]) -> Projection {
    let ids: HashMap<&str, &DocumentNodeId> = entries
        .iter()
        .map(|entry| (entry.path.as_str(), &entry.node_id))
        .collect();
    for node in &mut projection.root {
        relabel(node, &ids);
    }
    projection
}

fn relabel(node: &mut ProjectionNode, ids: &HashMap<&str, &DocumentNodeId>) {
    if let Some(id) = ids.get(node.id.as_str()) {
        let path = std::mem::replace(&mut node.id, (*id).clone());
        node.attrs.insert(PATH_ATTR.to_owned(), path.to_string());
    }
    for child in &mut node.children {
        relabel(child, ids);
    }
}

#[must_use]
pub fn path_of(node: &ProjectionNode) -> &str {
    node.attrs
        .get(PATH_ATTR)
        .map_or_else(|| node.id.as_str(), String::as_str)
}

#[must_use]
pub fn to_paths(ops: Vec<EditOp>, ids: &Projection) -> Vec<EditOp> {
    let paths: HashMap<&DocumentNodeId, DocumentNodeId> = ids
        .nodes()
        .map(|node| (&node.id, DocumentNodeId::new(path_of(node))))
        .collect();
    let path = |id: DocumentNodeId| paths.get(&id).cloned().unwrap_or(id);
    let target = |target: NodeTarget| match target {
        NodeTarget::Id(id) => NodeTarget::Id(path(id)),
        quote @ NodeTarget::Find(_) => quote,
    };
    ops.into_iter()
        .map(|op| match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                content_as,
            } => EditOp::ReplaceText {
                node: target(node),
                expected_hash,
                text,
                content_as,
            },
            EditOp::InsertAfter {
                node,
                kind,
                text,
                content_as,
            } => EditOp::InsertAfter {
                node: target(node),
                kind,
                text,
                content_as,
            },
            EditOp::Delete {
                node,
                expected_hash,
            } => EditOp::Delete {
                node: target(node),
                expected_hash,
            },
            EditOp::InsertSlide {
                after,
                layout,
                title,
                body,
            } => EditOp::InsertSlide {
                after: after.map(path),
                layout,
                title,
                body,
            },
            other => other,
        })
        .collect()
}
