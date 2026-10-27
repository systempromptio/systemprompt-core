//! Proposals carrying a whole body rather than edit ops: a revision written
//! off the head chain on a given base, for a reviewer to accept or reject.
//! Node ids carry forward from the base as they do for a byte commit, and a
//! newer proposal from the same proposer supersedes the open one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, ProposalStatus};

use super::commit::Landed;
use super::{Author, DocumentService, Proposed};
use crate::documents::error::DocumentsError;
use crate::documents::models::{NewProposal, ProposalReview};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposeBodyRequest {
    pub document_id: DocumentId,
    pub base_revision: DocumentRevisionId,
    pub bytes: Vec<u8>,
    pub author: Author,
    pub message: Option<String>,
}

impl DocumentService {
    pub async fn propose_body(
        &self,
        who: &AuthenticatedUser,
        request: ProposeBodyRequest,
        trace: &TraceId,
    ) -> Result<Proposed, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document_id, DocumentAction::Comment, trace)
            .await?;
        let base = self
            .resolve_revision(&document, Some(&request.base_revision))
            .await?;
        let (projected, _) = self.projected_with_bytes(&document, &base).await?;
        let (revision, _) = self
            .store_revision(
                who,
                &document,
                Landed {
                    bytes: &request.bytes,
                    retired: HashSet::new(),
                    previous: Some(projected.ids),
                    parent: Some(base.id.clone()),
                    edit_ops: None,
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
}
