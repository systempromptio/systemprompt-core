//! Applying ops to the head: quote targets resolve and node guards are
//! checked against the head's id-addressed projection, then the ops are
//! translated to codec paths and applied in one batch.
//!
//! An op that names a node by id must carry that node's `expected_hash`; a
//! quote target resolves its own. A guard names one node: an op is refused
//! only when that node's hash at the head differs, and an op naming an id
//! the head no longer has is refused as an unknown node. Codec errors are
//! relabelled from paths back to ids so a refusal names what the caller read.
//! A batch of rich-text and table ops applies through the document's model
//! instead of the codec, addressed by id, and may not mix with other ops.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{DocumentFormat, EditOp, NodeTarget, Projection};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, NodeFragment, resolve_targets,
};

use super::fragments::{FragmentInsert, FragmentPass, unsupported};
use super::node_ids::Projected;
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
use crate::documents::model::ops::apply_rich_ops_with;
use crate::documents::nodes::{CarryHints, path_of, to_paths};

#[derive(Debug, Clone)]
pub(super) struct AppliedOps {
    pub bytes: Vec<u8>,
    pub hints: CarryHints,
}

pub(super) struct HeadEdit {
    pub(super) ops: Vec<EditOp>,
    pub(super) fragments: Vec<NodeFragment>,
    pub(super) media: Media,
}

impl HeadEdit {
    pub(super) const fn ops(ops: Vec<EditOp>) -> Self {
        Self {
            ops,
            fragments: Vec::new(),
            media: Media::new(),
        }
    }
}

fn split_fragments(edit: HeadEdit) -> (Vec<EditOp>, Vec<FragmentInsert>, Media) {
    let mut fragments = edit.fragments.into_iter();
    let mut ops = Vec::with_capacity(edit.ops.len());
    let mut inserts = Vec::new();
    for op in edit.ops {
        match op {
            EditOp::InsertFromRevision { after, .. } => {
                if let Some(fragment) = fragments.next() {
                    inserts.push(FragmentInsert { after, fragment });
                }
            },
            other => ops.push(other),
        }
    }
    (ops, inserts, edit.media)
}

pub(super) fn apply_on_head(
    codec: &dyn DocumentCodec,
    format: DocumentFormat,
    bytes: &[u8],
    head: &Projected,
    edit: HeadEdit,
) -> Result<AppliedOps, DocumentsError> {
    let (ops, inserts, media) = split_fragments(edit);
    require_guards(&ops)?;
    let resolved = resolve_targets(&head.ids, ops)?;
    guard(&head.ids, &resolved.ops)?;
    let mut hints = CarryHints::default();
    for op in &resolved.ops {
        match op {
            EditOp::Delete { node, .. } => hints.retired.extend(node.id().cloned()),
            EditOp::ReplaceText { node, .. } => hints.rewritten.extend(node.id().cloned()),
            EditOp::SetMarks(_) | EditOp::SetBlockType(_) => {
                if let Some((node, _)) = op.rich_guard() {
                    hints.rewritten.extend(node.id().cloned());
                }
            },
            _ => {},
        }
    }
    let edited = if resolved.ops.is_empty() {
        bytes.to_vec()
    } else if resolved.ops.iter().any(EditOp::is_rich) {
        if !resolved.ops.iter().all(EditOp::is_rich) {
            return Err(DocumentsError::Codec(CodecError::Malformed {
                detail: "rich-text and table ops cannot share a batch with other ops",
            }));
        }
        apply_rich_ops_with(format, bytes, &head.ids, &resolved.ops, &media)
            .map_err(|error| relabel(DocumentsError::from(error), &head.ids))?
    } else {
        let ops = to_paths(resolved.ops, &head.ids);
        codec
            .apply(bytes, &head.paths, &ops)
            .map_err(|error| match error {
                CodecError::UnsupportedOp { .. } => unsupported(codec, format, error),
                other => relabel(DocumentsError::from(other), &head.ids),
            })?
    };
    let bytes = FragmentPass {
        codec,
        format,
        head,
        retired: &hints.retired,
    }
    .insert(edited, inserts)?;
    Ok(AppliedOps { bytes, hints })
}

fn require_guards(ops: &[EditOp]) -> Result<(), DocumentsError> {
    for op in ops {
        if let EditOp::ReplaceText {
            node: NodeTarget::Id(node),
            expected_hash: None,
            ..
        }
        | EditOp::Delete {
            node: NodeTarget::Id(node),
            expected_hash: None,
        } = op
        {
            return Err(DocumentsError::GuardMissing {
                op: op.name(),
                node: node.clone(),
            });
        }
    }
    Ok(())
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
            other => match other.rich_guard() {
                Some((node, expected)) => (node.id(), expected.map(String::as_str)),
                None => continue,
            },
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
