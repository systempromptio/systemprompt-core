//! Building a commit's bytes from its plan, expanding revision ops, and
//! answering a conflict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ContentAs, DocumentConflictEvent, EditOp, Projection,
};
use systemprompt_provider_contracts::documents::CodecError;

use super::apply::apply_on_head;
use super::byte_diff::DiffInput;
use super::commit::{Built, Landing};
use super::support::blocking;
use super::{Author, CommitOutcome, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::revision::{CommitBody, CommitPlan, MergeOutcome, three_way_bytes};

impl DocumentService {
    pub(super) async fn build(
        &self,
        plan: CommitPlan,
        landing: &Landing<'_>,
        body: CommitBody,
    ) -> Result<(Built, Option<Projection>), DocumentsError> {
        let document = landing.document;
        let head = match landing.head {
            Some(head) => Some(self.projected_with_bytes(document, head).await?),
            None => None,
        };
        let previous = head.as_ref().map(|(projected, _)| projected.ids.clone());
        let missing_head = || DocumentsError::not_found("head revision of document", &document.id);
        let built = match (plan, body) {
            (CommitPlan::NoHead, _) => return Err(missing_head()),
            (CommitPlan::Store, CommitBody::Bytes(bytes)) => Built::stored(bytes),
            (CommitPlan::ApplyOnHead, CommitBody::Ops(ops)) => {
                let (head, bytes) = head.ok_or_else(missing_head)?;
                let codec = self.codec(document.format)?;
                let format = document.format;
                Built::Applied(
                    blocking(move || apply_on_head(codec.as_ref(), format, &bytes, &head, ops))
                        .await?,
                )
            },
            (CommitPlan::MergeBytes, CommitBody::Bytes(ours)) => {
                let (_, head_bytes) = head.ok_or_else(missing_head)?;
                self.merge(landing, &ours, &head_bytes).await?
            },
            (CommitPlan::DiffOnHead, CommitBody::Bytes(uploaded)) => {
                let (head, head_bytes) = head.ok_or_else(missing_head)?;
                let input = DiffInput {
                    head,
                    head_bytes,
                    uploaded,
                };
                match self.diff_onto_head(document, landing.base, input).await? {
                    Some(applied) => Built::Applied(applied),
                    None => Built::Conflict,
                }
            },
            _ => Built::Conflict,
        };
        Ok((built, previous))
    }

    async fn merge(
        &self,
        landing: &Landing<'_>,
        ours: &[u8],
        head_bytes: &[u8],
    ) -> Result<Built, DocumentsError> {
        let base = self
            .resolve_revision(landing.document, landing.base)
            .await?;
        let base_bytes = self.revision_bytes(&base).await?;
        Ok(match three_way_bytes(&base_bytes, ours, head_bytes) {
            MergeOutcome::Clean(merged) => Built::stored(merged.into_bytes()),
            MergeOutcome::Conflicted => Built::Conflict,
        })
    }

    pub(super) async fn expand_op(
        &self,
        document: &Document,
        op: EditOp,
    ) -> Result<EditOp, DocumentsError> {
        match op {
            EditOp::Restore { .. } => Err(DocumentsError::Codec(CodecError::Malformed {
                detail: "restore must be the only op of its commit",
            })),
            EditOp::InsertFromRevision {
                revision_id,
                node,
                after,
            } => {
                let revision = self.resolve_revision(document, Some(&revision_id)).await?;
                let projected = self.projected(document, &revision).await?;
                let source = projected
                    .ids
                    .find(&node)
                    .ok_or(DocumentsError::UnknownNode { node })?;
                Ok(EditOp::InsertAfter {
                    node: after,
                    kind: source.kind,
                    text: source.text.clone().unwrap_or_default(),
                    content_as: ContentAs::Text,
                })
            },
            other => Ok(other),
        }
    }

    pub(super) async fn conflict(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        head: Option<DocumentRevision>,
        base: Option<DocumentRevisionId>,
        author: &Author,
    ) -> Result<CommitOutcome, DocumentsError> {
        let head = head
            .ok_or_else(|| DocumentsError::not_found("head revision of document", &document.id))?;
        let path = match self.document_path(document).await {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Conflict event carries the bare document name"
                );
                document.name.clone()
            },
        };
        let event = DocumentConflictEvent {
            document_id: document.id.clone(),
            path,
            head_revision_id: head.id.clone(),
            base_revision_id: base.clone(),
            device_id: author.origin.device().cloned(),
        };
        if let Err(error) = self
            .events
            .conflict(std::slice::from_ref(&who.id), &event)
            .await
        {
            tracing::warn!(
                document_id = %document.id,
                error = %error,
                "Conflict event not delivered"
            );
        }
        Ok(CommitOutcome::Conflict { head, base })
    }
}
