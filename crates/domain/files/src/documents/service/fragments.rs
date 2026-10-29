//! Inserting nodes copied natively from another revision
//! (`insert_from_revision` whose codec copies the node's own markup, runs
//! and tables included).
//!
//! The batch's other ops apply first. The copied nodes then go in after
//! their anchors, located by id in the edited bytes (ids carried from the
//! head), last anchor first so the positions of earlier anchors hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{
    ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection,
};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, NodeFragment, resolve_targets,
};

use super::node_ids::Projected;
use crate::documents::error::DocumentsError;
use crate::documents::nodes::{CarryHints, NodeIdMinter, carry_forward, path_of, with_ids};

const FRAGMENT_WRITER: &str = "~fragment";

#[derive(Debug, Clone)]
pub(super) struct FragmentInsert {
    pub(super) after: NodeTarget,
    pub(super) fragment: NodeFragment,
}

pub(super) fn unsupported(
    codec: &dyn DocumentCodec,
    format: DocumentFormat,
    error: CodecError,
) -> DocumentsError {
    match error {
        CodecError::UnsupportedOp { op, format: _ } => DocumentsError::UnsupportedOp {
            op,
            format,
            accepted_kinds: codec.insert_kinds(format),
        },
        other => DocumentsError::from(other),
    }
}

fn anchor_id(head: &Projection, after: &NodeTarget) -> Result<DocumentNodeId, DocumentsError> {
    let resolved = resolve_targets(
        head,
        vec![EditOp::InsertAfter {
            node: after.clone(),
            kind: NodeKind::Paragraph,
            text: String::new(),
            content_as: ContentAs::Text,
        }],
    )?;
    match resolved.ops.into_iter().next() {
        Some(EditOp::InsertAfter {
            node: NodeTarget::Id(id),
            ..
        }) => Ok(id),
        _ => Err(DocumentsError::FindNone {
            find: String::new(),
            closest: None,
        }),
    }
}

pub(super) struct FragmentPass<'a> {
    pub(super) codec: &'a dyn DocumentCodec,
    pub(super) format: DocumentFormat,
    pub(super) head: &'a Projected,
    pub(super) retired: &'a HashSet<DocumentNodeId>,
}

impl FragmentPass<'_> {
    pub(super) fn insert(
        &self,
        bytes: Vec<u8>,
        inserts: Vec<FragmentInsert>,
    ) -> Result<Vec<u8>, DocumentsError> {
        if inserts.is_empty() {
            return Ok(bytes);
        }
        let mut anchored = Vec::with_capacity(inserts.len());
        for insert in inserts {
            let id = anchor_id(&self.head.ids, &insert.after)?;
            if self.head.ids.find(&id).is_none() || self.retired.contains(&id) {
                return Err(DocumentsError::UnknownNode { node: id });
            }
            anchored.push((id, insert.fragment));
        }
        let current = self.codec.project(&bytes)?;
        let mut minter = NodeIdMinter::new(FRAGMENT_WRITER.to_owned(), 0);
        let hints = CarryHints {
            retired: self.retired.clone(),
            rewritten: HashSet::new(),
        };
        let entries = carry_forward(&self.head.ids, &current, &hints, &mut minter);
        let ids = with_ids(current.clone(), &entries);
        let order: Vec<&DocumentNodeId> = ids.nodes().map(|node| &node.id).collect();
        let mut placed = Vec::with_capacity(anchored.len());
        for (index, (id, fragment)) in anchored.into_iter().enumerate() {
            let position = order
                .iter()
                .position(|candidate| **candidate == id)
                .ok_or_else(|| DocumentsError::UnknownNode { node: id.clone() })?;
            let path = ids
                .find(&id)
                .map(|node| DocumentNodeId::new(path_of(node)))
                .ok_or_else(|| DocumentsError::UnknownNode { node: id.clone() })?;
            placed.push((position, index, path, fragment));
        }
        placed.sort_by_key(|entry| std::cmp::Reverse((entry.0, entry.1)));
        let mut bytes = bytes;
        for (_, _, path, fragment) in placed {
            bytes = self
                .codec
                .insert_fragment(&bytes, &current, &NodeTarget::Id(path), &fragment)
                .map_err(|error| unsupported(self.codec, self.format, error))?;
        }
        Ok(bytes)
    }
}
