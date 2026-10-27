//! Proposals: an edit written as a revision off the head chain for a
//! reviewer to accept or reject.
//!
//! The edit mode of a call is decided once: an explicit `land` needs `edit`
//! access; a caller holding `comment` but not `edit` always proposes; an
//! agent proposes when the document's folder, or any folder above it, sets
//! the `propose` agent edit policy. A proposed revision's parent is its
//! base, its ops are applied under the same node guards as a commit on the
//! head, and its node ids carry forward from the base; the head never moves.
//! A newer proposal from the same proposer on the same document supersedes
//! the open one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ActorKind, AgentEditPolicy, DocumentAction, EditMode, EditOp, ProposalStatus,
};

use super::apply::apply_on_head;
use super::commit::Landed;
use super::support::blocking;
use super::{Author, DocumentService};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{
    Document, DocumentProposal, DocumentRevision, NewProposal, ProposalReview,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ProposeRequest {
    pub document_id: DocumentId,
    pub base_revision: Option<DocumentRevisionId>,
    pub ops: Vec<EditOp>,
    pub author: Author,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    pub proposal: DocumentProposal,
    pub revision: DocumentRevision,
    pub superseded: Vec<DocumentProposal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditModeRequest<'a> {
    pub document: &'a DocumentId,
    pub requested: Option<EditMode>,
    pub actor: ActorKind,
}

impl DocumentService {
    pub async fn edit_mode(
        &self,
        who: &AuthenticatedUser,
        request: EditModeRequest<'_>,
        trace: &TraceId,
    ) -> Result<EditMode, DocumentsError> {
        let granted = self
            .access
            .probe(
                who,
                AccessTarget::Document(request.document.clone()),
                DocumentAction::Comment,
                trace,
            )
            .await?;
        let can_land = granted.level >= AccessLevel::Edit;
        match request.requested {
            Some(EditMode::Land) if !can_land => Err(DocumentsError::Forbidden {
                action: DocumentAction::Edit,
                needed: AccessLevel::Edit,
                held: Some(granted.level),
                policy: None,
                owner: granted.document.map(|document| document.owner_user_id),
            }),
            Some(mode) => Ok(mode),
            None if !can_land => Ok(EditMode::Propose),
            None if request.actor == ActorKind::Agent => {
                let folder = granted.document.and_then(|document| document.folder_id);
                let policy = match folder {
                    Some(folder) => {
                        self.repos
                            .folders
                            .effective_agent_edit_policy(&folder)
                            .await?
                    },
                    None => AgentEditPolicy::Direct,
                };
                Ok(match policy {
                    AgentEditPolicy::Propose => EditMode::Propose,
                    AgentEditPolicy::Direct => EditMode::Land,
                })
            },
            None => Ok(EditMode::Land),
        }
    }

    pub async fn propose(
        &self,
        who: &AuthenticatedUser,
        request: ProposeRequest,
        trace: &TraceId,
    ) -> Result<Proposed, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document_id, DocumentAction::Comment, trace)
            .await?;
        let base = self
            .resolve_revision(&document, request.base_revision.as_ref())
            .await?;
        let mut ops = Vec::with_capacity(request.ops.len());
        for op in request.ops {
            ops.push(self.expand_op(&document, op).await?);
        }
        let (projected, bytes) = self.projected_with_bytes(&document, &base).await?;
        let codec = self.codec(document.format)?;
        let format = document.format;
        let previous = projected.ids.clone();
        let applied_ops = ops.clone();
        let applied = blocking(move || {
            apply_on_head(codec.as_ref(), format, &bytes, &projected, applied_ops)
        })
        .await?;
        let (revision, _) = self
            .store_revision(
                who,
                &document,
                Landed {
                    bytes: &applied.bytes,
                    retired: applied.retired,
                    previous: Some(previous),
                    parent: Some(base.id.clone()),
                    edit_ops: Some(ops),
                    message: request.message.clone(),
                    author: &request.author,
                },
            )
            .await?;
        let superseded = self
            .repos
            .proposals
            .close_open(
                &document.id,
                Some(&who.id),
                &ProposalReview {
                    status: ProposalStatus::Superseded,
                    reviewer_user_id: None,
                    note: None,
                    landed_revision_id: None,
                },
            )
            .await?;
        let proposal = self
            .repos
            .proposals
            .insert(NewProposal {
                document_id: document.id.clone(),
                base_revision_id: base.id,
                proposed_revision_id: revision.id.clone(),
                proposer_user_id: who.id.clone(),
                actor_kind: request.author.actor_kind,
                origin_kind: request.author.origin.kind(),
                origin_id: request.author.origin.id(),
                message: request.message,
            })
            .await?;
        for closed in &superseded {
            self.announce_proposal(&document, closed).await;
        }
        self.announce_proposal(&document, &proposal).await;
        Ok(Proposed {
            proposal,
            revision,
            superseded,
        })
    }

    pub async fn reject_open_for_document(
        &self,
        document: &Document,
        note: Option<String>,
    ) -> Result<Vec<DocumentProposal>, DocumentsError> {
        let rejected = self
            .repos
            .proposals
            .close_open(
                &document.id,
                None,
                &ProposalReview {
                    status: ProposalStatus::Rejected,
                    reviewer_user_id: None,
                    note,
                    landed_revision_id: None,
                },
            )
            .await?;
        for proposal in &rejected {
            self.announce_proposal(document, proposal).await;
        }
        Ok(rejected)
    }
}
