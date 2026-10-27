//! Reading proposals, the folder agent edit policy, and the
//! `document.proposal.*` events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentProposalId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AgentEditPolicy, DocumentAction, DocumentProposalEvent, Projection, ProposalStatus,
};

use super::DocumentService;
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, DocumentProposal};

const PROPOSAL_LIMIT: i64 = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct ProposalView {
    pub proposal: DocumentProposal,
    pub base: Projection,
    pub proposed: Projection,
}

impl DocumentService {
    pub async fn list_proposals(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        status: Option<ProposalStatus>,
        trace: &TraceId,
    ) -> Result<Vec<DocumentProposal>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        Ok(self
            .repos
            .proposals
            .list(document, status, PROPOSAL_LIMIT)
            .await?)
    }

    pub async fn open_proposals_for(
        &self,
        who: &AuthenticatedUser,
    ) -> Result<Vec<DocumentProposal>, DocumentsError> {
        Ok(self
            .repos
            .proposals
            .open_for_user(&who.id, PROPOSAL_LIMIT)
            .await?)
    }

    pub async fn proposal(
        &self,
        who: &AuthenticatedUser,
        proposal: &DocumentProposalId,
        trace: &TraceId,
    ) -> Result<DocumentProposal, DocumentsError> {
        Ok(self.visible_proposal(who, proposal, trace).await?.1)
    }

    async fn visible_proposal(
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
            .authorized_document(who, &proposal.document_id, DocumentAction::View, trace)
            .await?;
        Ok((document, proposal))
    }

    pub async fn proposal_view(
        &self,
        who: &AuthenticatedUser,
        proposal: &DocumentProposalId,
        trace: &TraceId,
    ) -> Result<ProposalView, DocumentsError> {
        let (document, proposal) = self.visible_proposal(who, proposal, trace).await?;
        let base = self
            .resolve_revision(&document, Some(&proposal.base_revision_id))
            .await?;
        let proposed = self
            .proposed_revision(&document, &proposal.proposed_revision_id)
            .await?;
        Ok(ProposalView {
            base: self.projected(&document, &base).await?.ids,
            proposed: self.projected(&document, &proposed).await?.ids,
            proposal,
        })
    }

    pub async fn set_folder_agent_edit_policy(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        policy: AgentEditPolicy,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::Share,
                trace,
            )
            .await?;
        if !self
            .repos
            .folders
            .set_agent_edit_policy(folder, policy)
            .await?
        {
            return Err(DocumentsError::not_found("folder", folder));
        }
        self.load_folder(folder).await
    }

    pub(super) async fn announce_proposal(
        &self,
        document: &Document,
        proposal: &DocumentProposal,
    ) -> bool {
        let event = DocumentProposalEvent {
            document_id: document.id.clone(),
            proposal_id: proposal.id.clone(),
            status: proposal.status,
            proposer_id: proposal.proposer_user_id.clone(),
            base_revision_id: proposal.base_revision_id.clone(),
            proposed_revision_id: proposal.proposed_revision_id.clone(),
            reviewer_id: proposal.reviewer_user_id.clone(),
            landed_revision_id: proposal.landed_revision_id.clone(),
            message: proposal.message.clone(),
        };
        let mut to = self.recipients(document).await;
        if !to.contains(&proposal.proposer_user_id) {
            to.push(proposal.proposer_user_id.clone());
        }
        self.events.proposal(&to, &event).await.is_ok()
    }
}
