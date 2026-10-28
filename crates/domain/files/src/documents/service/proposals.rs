//! Proposals: an edit written as a revision off the head chain for a
//! reviewer to accept or reject.
//!
//! The edit mode of a call is decided once: an explicit `land` needs `edit`
//! access; a caller holding `comment` but not `edit` always proposes; an
//! agent proposes when the document's folder, or any folder above it, sets
//! the `propose` agent edit policy, and an agent's explicit `land` there is
//! refused, since the folder policy outranks the request. A proposed revision's
//! parent is its base, its ops are applied under the same node guards as a
//! commit on the head, and its node ids carry forward from the base; the head
//! never moves. A newer proposal from the same proposer on the same document
//! supersedes the open one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ActorKind, AgentEditPolicy, DocumentAction, EditMode, EditOp, ProposalStatus,
};

use super::apply::{HeadEdit, apply_on_head};
use super::landing::{LandResult, Placement};
use super::prepare::{Applied, Draft};
use super::support::blocking;
use super::{Author, CommentPolicy, DocumentService};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
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
        let held = granted.level;
        let document = granted.document;
        let forbidden =
            |policy: Option<String>, document: Option<Document>| DocumentsError::Forbidden {
                action: DocumentAction::Edit,
                needed: AccessLevel::Edit,
                held: Some(held),
                policy,
                owner: document.map(|document| document.owner_user_id),
            };
        if !can_land {
            return match request.requested {
                Some(EditMode::Land) => Err(forbidden(None, document)),
                _ => Ok(EditMode::Propose),
            };
        }
        if request.actor != ActorKind::Agent {
            return Ok(request.requested.unwrap_or(EditMode::Land));
        }
        let policy = match document
            .as_ref()
            .and_then(|document| document.folder_id.as_ref())
        {
            Some(folder) => {
                self.repos
                    .folders
                    .effective_agent_edit_policy(folder)
                    .await?
            },
            None => AgentEditPolicy::Direct,
        };
        match (policy, request.requested) {
            (AgentEditPolicy::Propose, Some(EditMode::Land)) => {
                Err(forbidden(Some("propose".to_owned()), document))
            },
            (AgentEditPolicy::Propose, _) => Ok(EditMode::Propose),
            (AgentEditPolicy::Direct, requested) => Ok(requested.unwrap_or(EditMode::Land)),
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
        self.check_ops_limits(&request.ops)?;
        let applied = self.apply_proposed(&document, &base, request.ops).await?;
        let prepared = self
            .prepare(
                who,
                &document,
                Draft {
                    applied,
                    parent: Some(base.id.clone()),
                    message: request.message.clone(),
                    author: &request.author,
                },
            )
            .await?;
        let revision = match self
            .land(
                &document,
                prepared,
                Placement::proposal(&CommentPolicy::default()),
            )
            .await?
        {
            LandResult::Landed(stored) => stored.revision,
            LandResult::HeadMoved | LandResult::Claimed(_) => {
                return Err(DocumentsError::not_found("proposed revision", &document.id));
            },
        };
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

    async fn apply_proposed(
        &self,
        document: &Document,
        base: &DocumentRevision,
        requested: Vec<EditOp>,
    ) -> Result<Applied, DocumentsError> {
        let mut ops = Vec::with_capacity(requested.len());
        let mut fragments = Vec::new();
        for op in requested {
            let (op, fragment) = self.expand_op(document, op).await?;
            ops.push(op);
            fragments.extend(fragment);
        }
        let (projected, bytes) = self.projected_with_bytes(document, base).await?;
        let codec = self.codec(document.format)?;
        let format = document.format;
        let previous = projected.ids.clone();
        let applied_ops = ops.clone();
        let applied = blocking(move || {
            let edit = HeadEdit {
                ops: applied_ops,
                fragments,
                media: Media::new(),
            };
            apply_on_head(codec.as_ref(), format, &bytes, &projected, edit)
        })
        .await?;
        Ok(Applied {
            ops: applied,
            previous: Some(previous),
            merged: false,
            edit_ops: Some(ops),
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
