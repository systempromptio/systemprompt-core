//! Preparing a commit for landing: building the new bytes from the plan,
//! writing the blob, projecting the new revision's paths, and assembling
//! the revision row with a fresh id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{EditOp, Projection};
use systemprompt_provider_contracts::documents::NodeFragment;

use super::apply::AppliedOps;
use super::commit::{Built, Landing};
use super::landing::Prepared;
#[cfg(feature = "office")]
use super::support::blocking;
use super::{Author, DocumentService};
#[cfg(feature = "office")]
use crate::documents::codecs::inspect_package;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, NewRevision};
use crate::documents::revision::{CommitBody, CommitPlan};

pub(super) struct Applied {
    pub(super) ops: AppliedOps,
    pub(super) previous: Option<Projection>,
    pub(super) merged: bool,
    pub(super) edit_ops: Option<Vec<EditOp>>,
}

pub(super) struct Draft<'a> {
    pub(super) applied: Applied,
    pub(super) parent: Option<DocumentRevisionId>,
    pub(super) message: Option<String>,
    pub(super) author: &'a Author,
}

impl DocumentService {
    pub(super) async fn apply_body(
        &self,
        landing: &Landing<'_>,
        body: CommitBody,
        fragments: Vec<NodeFragment>,
    ) -> Result<Option<Applied>, DocumentsError> {
        let head_id = landing.head.map(|h| &h.id);
        let plan = CommitPlan::decide(head_id, landing.base, landing.document.format, &body);
        let merged = plan.merges(head_id, landing.base);
        let edit_ops = match &body {
            CommitBody::Ops(ops) => Some(ops.clone()),
            CommitBody::Bytes(_) => None,
        };
        let (built, previous) = self.build(plan, landing, body, fragments).await?;
        Ok(match built {
            Built::Applied(ops) => Some(Applied {
                ops,
                previous,
                merged,
                edit_ops,
            }),
            Built::Conflict => None,
        })
    }

    pub(super) async fn prepare(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        draft: Draft<'_>,
    ) -> Result<Prepared, DocumentsError> {
        let Draft {
            applied,
            parent,
            message,
            author,
        } = draft;
        let bytes = applied.ops.bytes;
        self.check_size(bytes.len())?;
        #[cfg(feature = "office")]
        let bytes = if applied.edit_ops.is_none() {
            let caps = self.package_caps();
            blocking(move || {
                inspect_package(&bytes, caps)?;
                Ok(bytes)
            })
            .await?
        } else {
            bytes
        };
        let blob = self.blobs.put(&bytes).await?;
        let next = self.project_paths(document.format, bytes).await?;
        let count = applied.edit_ops.as_ref().map_or(0, Vec::len);
        Ok(Prepared {
            revision: NewRevision {
                id: DocumentRevisionId::generate(),
                document_id: document.id.clone(),
                parent_revision_id: parent.clone(),
                content_sha256: blob.hash,
                author_user_id: who.id.clone(),
                actor_kind: author.actor_kind,
                origin_kind: author.origin.kind(),
                origin_id: author.origin.id(),
                message,
                edit_ops: applied.edit_ops,
                attribution: author.attribution.clone(),
            },
            next,
            previous: applied.previous,
            hints: applied.ops.hints,
            merged_from: parent.filter(|_| applied.merged),
            applied: count,
        })
    }
}
