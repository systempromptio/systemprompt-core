//! Applying ops to the head: quote targets resolve and node guards are
//! checked against the head's id-addressed projection, then the ops are
//! translated to codec paths and applied in one batch.
//!
//! A guard names one node: an op that quotes a hash is refused only when
//! that node's hash at the head differs, and an op naming an id the head no
//! longer has is refused as an unknown node. Codec errors are relabelled
//! from paths back to ids so a refusal names what the caller read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{DocumentFormat, EditOp, Projection};
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, resolve_targets};

use super::node_ids::Projected;
use crate::documents::error::DocumentsError;
use crate::documents::nodes::{path_of, to_paths};

#[derive(Debug, Clone)]
pub(super) struct AppliedOps {
    pub bytes: Vec<u8>,
    pub retired: HashSet<DocumentNodeId>,
}

pub(super) fn apply_on_head(
    codec: &dyn DocumentCodec,
    format: DocumentFormat,
    bytes: &[u8],
    head: &Projected,
    ops: Vec<EditOp>,
) -> Result<AppliedOps, DocumentsError> {
    let resolved = resolve_targets(&head.ids, ops)?;
    guard(&head.ids, &resolved.ops)?;
    let retired = resolved
        .ops
        .iter()
        .filter_map(|op| match op {
            EditOp::Delete { node, .. } => node.id().cloned(),
            _ => None,
        })
        .collect();
    let ops = to_paths(resolved.ops, &head.ids);
    let bytes = codec
        .apply(bytes, &head.paths, &ops)
        .map_err(|error| match error {
            CodecError::UnsupportedOp { op, format: _ } => DocumentsError::UnsupportedOp {
                op,
                format,
                accepted_kinds: codec.insert_kinds(format),
            },
            other => relabel(DocumentsError::from(other), &head.ids),
        })?;
    Ok(AppliedOps { bytes, retired })
}

pub(super) fn guard(ids: &Projection, ops: &[EditOp]) -> Result<(), DocumentsError> {
    for op in ops {
        let (node, expected) = match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                ..
            }
            | EditOp::Delete {
                node,
                expected_hash,
            } => (node.id(), expected_hash.as_deref()),
            EditOp::InsertAfter { node, .. } => (node.id(), None),
            EditOp::InsertSlide { after, .. } => (after.as_ref(), None),
            _ => continue,
        };
        let Some(node) = node else {
            continue;
        };
        let current = ids
            .find(node)
            .ok_or_else(|| DocumentsError::UnknownNode { node: node.clone() })?;
        if let Some(expected) = expected
            && expected != current.hash8
        {
            return Err(DocumentsError::StaleAnchor {
                node: node.clone(),
                expected: expected.to_owned(),
                actual: current.hash8.clone(),
                current_text: current.text.clone().unwrap_or_default(),
            });
        }
    }
    Ok(())
}

fn relabel(error: DocumentsError, ids: &Projection) -> DocumentsError {
    let by_path: HashMap<&str, &DocumentNodeId> =
        ids.nodes().map(|node| (path_of(node), &node.id)).collect();
    let id = |node: DocumentNodeId| {
        by_path
            .get(node.as_str())
            .map_or_else(|| node.clone(), |id| (*id).clone())
    };
    match error {
        DocumentsError::StaleAnchor {
            node,
            expected,
            actual,
            current_text,
        } => DocumentsError::StaleAnchor {
            node: id(node),
            expected,
            actual,
            current_text,
        },
        DocumentsError::UnknownNode { node } => DocumentsError::UnknownNode { node: id(node) },
        other => other,
    }
}
