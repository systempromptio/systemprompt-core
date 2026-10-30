//! Reviewing a proposal: accept or reject.
//!
//! Accepting claims the proposal first (`open` to `accepted`), then lands
//! it. When the head is still the proposal's base, the proposed revision
//! itself becomes the head by compare-and-swap and keeps the proposer as
//! its author. When the head has moved, the proposal's ops are applied
//! again on the head under node guards as a commit by the reviewer, on
//! behalf of the proposer; a touched node that changed refuses as
//! [`DocumentsError::ProposalConflict`] with its current text. Either way
//! the landing transaction records the landed revision on the proposal, so
//! an accepted proposal is landed exactly when it names its revision. A
//! landing that fails returns the proposal to `open`; one interrupted
//! before it settles is reopened by the presence sweep. A proposer cannot
//! accept their own proposal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentProposalId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, ProposalStatus};

use super::{Author, CommentPolicy, CommitOutcome, CommitRequest, DocumentService};
use crate::documents::agents::AgentCapability;
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
use crate::documents::models::{Document, DocumentProposal, DocumentRevision, ProposalReview};
use crate::documents::revision::CommitBody;

struct Landing<'a> {
    proposal: &'a DocumentProposal,
    reviewer: Author,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    pub proposal_id: DocumentProposalId,
    pub note: Option<String>,
    pub reviewer: Author,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Accepted {
    pub proposal: DocumentProposal,
    pub revision: DocumentRevision,
    pub rebased: bool,
}

impl DocumentService {
    pub async fn accept_proposal(
        &self,
        who: &AuthenticatedUser,
        request: ReviewRequest,
        trace: &TraceId,
    ) -> Result<Accepted, DocumentsError> {
        let (document, proposal) = self.reviewable(who, &request.proposal_id, trace).await?;
        if proposal.proposer_user_id == who.id {
            return Err(DocumentsError::SelfReview {
                proposal_id: proposal.id,
            });
        }
        let claimed = self
            .claim(
                &proposal,
                ProposalReview {
                    status: ProposalStatus::Accepted,
                    reviewer_user_id: Some(who.id.clone()),
                    note: request.note,
                    landed_revision_id: None,
                },
            )
            .await?;
        let landed = self
            .land_proposal(
                who,
                &document,
                Landing {
                    proposal: &claimed,
                    reviewer: request.reviewer,
                },
                trace,
            )
            .await;
        let (revision, rebased) = match landed {
            Ok(landed) => landed,
            Err(error) => {
                self.repos.proposals.settle(&claimed.id, None).await?;
                return Err(error);
            },
        };
        let proposal = DocumentProposal {
            landed_revision_id: Some(revision.id.clone()),
            ..claimed
        };
        self.announce_proposal(&document, &proposal).await;
        Ok(Accepted {
            proposal,
            revision,
            rebased,
        })
    }

    pub async fn reject_proposal(
        &self,
        who: &AuthenticatedUser,
        request: ReviewRequest,
        trace: &TraceId,
    ) -> Result<DocumentProposal, DocumentsError> {
        let (document, proposal) = self.reviewable(who, &request.proposal_id, trace).await?;
        let rejected = self
            .claim(
                &proposal,
                ProposalReview {
                    status: ProposalStatus::Rejected,
                    reviewer_user_id: Some(who.id.clone()),
                    note: request.note,
                    landed_revision_id: None,
                },
            )
            .await?;
        self.announce_proposal(&document, &rejected).await;
        Ok(rejected)
    }

    async fn reviewable(
        &self,
        who: &AuthenticatedUser,
        proposal: &DocumentProposalId,
        trace: &TraceId,
    ) -> Result<(Document, DocumentProposal), DocumentsError> {
        let proposal = self
            .repos
            .proposals
            .find(proposal)
            .await?
            .ok_or_else(|| DocumentsError::not_found("proposal", proposal))?;
        let document = self
            .authorized_document(who, &proposal.document_id, DocumentAction::Edit, trace)
            .await?;
        self.access
            .require_agent_capability(who, &document, AgentCapability::ReviewProposal)
            .await?;
        Ok((document, proposal))
    }

    async fn claim(
        &self,
        proposal: &DocumentProposal,
        review: ProposalReview,
    ) -> Result<DocumentProposal, DocumentsError> {
        if proposal.status != ProposalStatus::Open {
            return Err(DocumentsError::ProposalClosed {
                proposal_id: proposal.id.clone(),
                status: proposal.status,
            });
        }
        if let Some(claimed) = self.repos.proposals.review(&proposal.id, &review).await? {
            return Ok(claimed);
        }
        let status = self
            .repos
            .proposals
            .find(&proposal.id)
            .await?
            .map_or(ProposalStatus::Rejected, |current| current.status);
        Err(DocumentsError::ProposalClosed {
            proposal_id: proposal.id.clone(),
            status,
        })
    }

    async fn land_proposal(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        Landing { proposal, reviewer }: Landing<'_>,
        trace: &TraceId,
    ) -> Result<(DocumentRevision, bool), DocumentsError> {
        let mut proposed = self
            .proposed_revision(document, &proposal.proposed_revision_id)
            .await?;
        let head = self.resolve_revision(document, None).await?;
        if head.id == proposal.base_revision_id
            && self
                .land_existing(document, &head.id, &mut proposed, &proposal.id)
                .await?
        {
            let bytes = self.revision_bytes(&proposed).await?;
            self.announce_landed(document, &proposed, bytes).await;
            return Ok((proposed, false));
        }
        let conflict = |head: DocumentRevision| DocumentsError::ProposalConflict {
            proposal_id: proposal.id.clone(),
            head: head.id,
            node: None,
            current_text: None,
        };
        let Some(ops) = proposed.edit_ops.clone() else {
            return Err(conflict(head));
        };
        let mut author = reviewer;
        author.attribution.on_behalf_of = Some(proposal.proposer_user_id.clone());
        let outcome = self
            .commit_settling(
                who,
                CommitRequest {
                    document_id: document.id.clone(),
                    base_revision: Some(head.id.clone()),
                    body: CommitBody::Ops(ops),
                    author,
                    message: proposal.message.clone(),
                    lock_holder: None,
                    comments: CommentPolicy::default(),
                    op_id: None,
                    media: Media::new(),
                },
                Some(&proposal.id),
                trace,
            )
            .await
            .map_err(|error| rebase_refusal(error, proposal, &head))?;
        match outcome {
            CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => {
                Ok((landed.revision, true))
            },
            CommitOutcome::Conflict { head, .. } => Err(conflict(*head)),
        }
    }
}

fn rebase_refusal(
    error: DocumentsError,
    proposal: &DocumentProposal,
    head: &DocumentRevision,
) -> DocumentsError {
    let (node, current_text) = match error {
        DocumentsError::StaleAnchor {
            node, current_text, ..
        } => (node, Some(current_text)),
        DocumentsError::UnknownNode { node } => (node, None),
        other => return other,
    };
    DocumentsError::ProposalConflict {
        proposal_id: proposal.id.clone(),
        head: head.id.clone(),
        node: Some(node),
        current_text,
    }
}
