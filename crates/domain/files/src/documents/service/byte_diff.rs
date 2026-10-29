//! Landing a whole Office file uploaded on a stale base: the upload is
//! diffed against its base into ops ([`derive_ops`]), the ops are checked to
//! rebuild the upload's blocks from the base, text and formatting alike
//! (compared by the codec's per-leaf style hashes, so a formatting change
//! the ops cannot carry is not dropped), and then they apply to the head
//! under the same node guards as any edit. Anything the diff cannot express,
//! or a guard the head trips, is a conflict (`None`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{NodeKind, Projection};

use super::DocumentService;
use super::apply::{AppliedOps, HeadEdit, apply_on_head};
use super::node_ids::Projected;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::nodes::{
    CarryHints, NodeIdMinter, carry_forward, derive_ops, flatten, to_paths, with_ids,
};

const UPLOAD_WRITER: &str = "~upload";

pub(super) struct DiffInput {
    pub head: Projected,
    pub head_bytes: Vec<u8>,
    pub uploaded: Vec<u8>,
}

fn leaves(projection: &Projection) -> Vec<(NodeKind, String)> {
    flatten(projection)
        .into_iter()
        .filter(|flat| flat.node.children.is_empty())
        .map(|flat| (flat.node.kind, flat.node.hash8.clone()))
        .collect()
}

fn styles(hashes: &[(DocumentNodeId, String)]) -> Vec<&str> {
    hashes.iter().map(|(_, hash)| hash.as_str()).collect()
}

const fn inexpressible(error: &DocumentsError) -> bool {
    matches!(
        error,
        DocumentsError::StaleAnchor { .. }
            | DocumentsError::UnknownNode { .. }
            | DocumentsError::UnsupportedOp { .. }
            | DocumentsError::FindNone { .. }
            | DocumentsError::Codec(_)
    )
}

impl DocumentService {
    pub(super) async fn diff_onto_head(
        &self,
        document: &Document,
        base: Option<&DocumentRevisionId>,
        input: DiffInput,
    ) -> Result<Option<AppliedOps>, DocumentsError> {
        let base_revision = self.resolve_revision(document, base).await?;
        let (base, base_bytes) = self.projected_with_bytes(document, &base_revision).await?;
        let codec = self.codec(document.format)?;
        let format = document.format;
        blocking(move || {
            let uploaded = codec.project(&input.uploaded)?;
            let mut minter = NodeIdMinter::new(UPLOAD_WRITER.to_owned(), 0);
            let entries = carry_forward(&base.ids, &uploaded, &CarryHints::default(), &mut minter);
            let carried = with_ids(uploaded.clone(), &entries);
            let Some(ops) = derive_ops(&base.ids, &carried) else {
                return Ok(None);
            };
            let Ok(rebuilt) =
                codec.apply(&base_bytes, &base.paths, &to_paths(ops.clone(), &base.ids))
            else {
                return Ok(None);
            };
            if leaves(&codec.project(&rebuilt)?) != leaves(&uploaded)
                || styles(&codec.style_hashes(&rebuilt)?)
                    != styles(&codec.style_hashes(&input.uploaded)?)
            {
                return Ok(None);
            }
            let edit = HeadEdit::ops(ops);
            match apply_on_head(codec.as_ref(), format, &input.head_bytes, &input.head, edit) {
                Ok(applied) => Ok(Some(applied)),
                Err(error) if inexpressible(&error) => Ok(None),
                Err(error) => Err(error),
            }
        })
        .await
    }
}
