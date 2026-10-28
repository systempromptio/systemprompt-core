//! Proposal rows and the proposal insert request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, UserId};
use systemprompt_models::documents::{ActorKind, ProposalStatus, RevisionOriginKind};

/// A reviewable revision: `proposed_revision_id` branches off
/// `base_revision_id` and never becomes the head itself unless accepted
/// while the head is still the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentProposal {
    pub id: DocumentProposalId,
    pub document_id: DocumentId,
    pub base_revision_id: DocumentRevisionId,
    pub proposed_revision_id: DocumentRevisionId,
    pub proposer_user_id: UserId,
    pub actor_kind: ActorKind,
    pub origin_kind: RevisionOriginKind,
    pub origin_id: Option<String>,
    pub status: ProposalStatus,
    pub message: Option<String>,
    pub reviewer_user_id: Option<UserId>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub landed_revision_id: Option<DocumentRevisionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewProposal {
    pub document_id: DocumentId,
    pub base_revision_id: DocumentRevisionId,
    pub proposed_revision_id: DocumentRevisionId,
    pub proposer_user_id: UserId,
    pub actor_kind: ActorKind,
    pub origin_kind: RevisionOriginKind,
    pub origin_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProposalReview {
    pub status: ProposalStatus,
    pub reviewer_user_id: Option<UserId>,
    pub note: Option<String>,
    pub landed_revision_id: Option<DocumentRevisionId>,
}
