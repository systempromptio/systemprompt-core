//! Building a commit's bytes from its plan, expanding revision ops, and
//! answering a conflict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{ContentAs, DocumentConflictEvent, EditOp, Projection};
use systemprompt_provider_contracts::documents::{CodecError, NodeFragment};

use super::apply::{HeadEdit, apply_on_head};
use super::byte_diff::DiffInput;
use super::commit::{Built, Landing};
use super::fragments::unsupported;
use super::support::blocking;
use super::{Author, CommitOutcome, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::nodes::path_of;
use crate::documents::revision::{CommitBody, CommitPlan, MergeOutcome, three_way_bytes};

pub(super) struct Conflicted<'a> {
    pub(super) head: Option<DocumentRevision>,
    pub(super) base: Option<DocumentRevisionId>,
    pub(super) author: &'a Author,
}

impl DocumentService {
    pub(super) async fn build(
        &self,
        plan: CommitPlan,
        landing: &Landing<'_>,
        body: CommitBody,
        fragments: Vec<NodeFragment>,
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
                let media = landing.media.clone();
                Built::Applied(
                    blocking(move || {
                        let edit = HeadEdit {
                            ops,
                            fragments,
                            media,
                        };
                        apply_on_head(codec.as_ref(), format, &bytes, &head, edit)
                    })
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
                self.diff_onto_head(document, landing.base, input)
                    .await?
                    .map_or(Built::Conflict, Built::Applied)
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
        let base_bytes = self.parent_bytes(&base).await?;
        Ok(match three_way_bytes(&base_bytes, ours, head_bytes) {
            MergeOutcome::Clean(merged) => Built::stored(merged.into_bytes()),
            MergeOutcome::Conflicted => Built::Conflict,
        })
    }

    pub(super) async fn expand_op(
        &self,
        document: &Document,
        op: EditOp,
    ) -> Result<(EditOp, Option<NodeFragment>), DocumentsError> {
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
                let (projected, bytes) = self.projected_with_bytes(document, &revision).await?;
                let path = projected
                    .ids
                    .find(&node)
                    .map(|source| DocumentNodeId::new(path_of(source)))
                    .ok_or_else(|| DocumentsError::UnknownNode { node: node.clone() })?;
                let codec = self.codec(document.format)?;
                let format = document.format;
                let paths = projected.paths;
                let fragment = blocking(move || {
                    codec
                        .copy_node(&bytes, &paths, &path)
                        .map_err(|error| unsupported(codec.as_ref(), format, error))
                })
                .await?;
                if fragment.native.is_some() {
                    return Ok((
                        EditOp::InsertFromRevision {
                            revision_id,
                            node,
                            after,
                        },
                        Some(fragment),
                    ));
                }
                Ok((
                    EditOp::InsertAfter {
                        node: after,
                        kind: fragment.kind,
                        text: fragment.text.unwrap_or_default(),
                        content_as: ContentAs::Text,
                    },
                    None,
                ))
            },
            other => Ok((other, None)),
        }
    }

    pub(super) async fn conflict(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        Conflicted { head, base, author }: Conflicted<'_>,
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
        Ok(CommitOutcome::Conflict {
            head: Box::new(head),
            base,
        })
    }
}
