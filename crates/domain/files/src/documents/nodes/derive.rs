//! Expressing an uploaded Office file as edit ops against the revision it
//! was based on, so a stale whole-file upload can land on a newer head.
//!
//! Both projections carry ids: the base's own, and the upload's ids carried
//! forward from the base (new blocks minted). Only leaf text blocks
//! (headings, paragraphs, list items) are expressed: a changed one becomes
//! `replace_text`, a removed one `delete`, a new one `insert_after` its
//! previous sibling. Anything else (a container added or removed, a leaf of
//! another kind changing, a kind change, a new first child) has no op form
//! and yields `None`, which the caller answers as a conflict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{
    ContentAs, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};

use super::flat::{FlatNode, flatten};

const fn editable(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Heading { .. } | NodeKind::Paragraph | NodeKind::ListItem
    )
}

fn text(node: &ProjectionNode) -> String {
    node.text.clone().unwrap_or_default()
}

#[must_use]
pub fn derive_ops(base: &Projection, uploaded: &Projection) -> Option<Vec<EditOp>> {
    let base_flat = flatten(base);
    let up_flat = flatten(uploaded);
    let base_nodes: HashMap<&DocumentNodeId, &ProjectionNode> =
        base_flat.iter().map(|f| (&f.node.id, f.node)).collect();
    let up_ids: HashSet<&DocumentNodeId> = up_flat.iter().map(|f| &f.node.id).collect();

    for flat in &base_flat {
        let node = flat.node;
        if !node.children.is_empty() && !up_ids.contains(&node.id) {
            return None;
        }
    }
    let mut ops = Vec::new();
    let mut anchors: HashMap<&DocumentNodeId, DocumentNodeId> = HashMap::new();
    for (index, flat) in up_flat.iter().enumerate() {
        let node = flat.node;
        if let Some(before) = base_nodes.get(&node.id) {
            if before.children.is_empty() != node.children.is_empty() {
                return None;
            }
            if !node.children.is_empty() || before.hash8 == node.hash8 {
                continue;
            }
            if before.kind != node.kind || !editable(node.kind) {
                return None;
            }
            ops.push(EditOp::ReplaceText {
                node: NodeTarget::Id(node.id.clone()),
                expected_hash: Some(before.hash8.clone()),
                text: text(node),
                content_as: ContentAs::Text,
            });
        } else {
            if !node.children.is_empty() || !editable(node.kind) {
                return None;
            }
            let sibling = previous_sibling(&up_flat, index)?;
            let anchor = if base_nodes.contains_key(&sibling.id) {
                sibling.id.clone()
            } else {
                anchors.get(&sibling.id)?.clone()
            };
            anchors.insert(&node.id, anchor.clone());
            ops.push(EditOp::InsertAfter {
                node: NodeTarget::Id(anchor),
                kind: node.kind,
                text: text(node),
                content_as: ContentAs::Text,
            });
        }
    }
    for flat in &base_flat {
        let node = flat.node;
        if node.children.is_empty() && !up_ids.contains(&node.id) {
            if !editable(node.kind) {
                return None;
            }
            ops.push(EditOp::Delete {
                node: NodeTarget::Id(node.id.clone()),
                expected_hash: Some(node.hash8.clone()),
            });
        }
    }
    (!ops.is_empty()).then_some(ops)
}

fn previous_sibling<'p>(flat: &[FlatNode<'p>], index: usize) -> Option<&'p ProjectionNode> {
    let parent = flat[index].parent;
    flat[..index]
        .iter()
        .rev()
        .find(|candidate| candidate.parent == parent)
        .map(|candidate| candidate.node)
}
