//! Proposals drafted by analyses: listed for review and approved or
//! rejected by an admin. Deciding a proposal only records the decision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::BrainProposalId;
use systemprompt_models::brain::{BrainProposal, ProposalStatus};

use super::{BrainCaller, BrainService, authority};
use crate::error::{BrainError, BrainResult};
use crate::repository::ProposalRepository;

impl BrainService {
    pub async fn proposals(
        &self,
        caller: &BrainCaller,
        limit: i64,
    ) -> BrainResult<Vec<BrainProposal>> {
        authority::may_decide_proposals(caller)?;
        Ok(self.repos.proposals.list_open(limit).await?)
    }

    pub async fn approve_proposal(
        &self,
        caller: &BrainCaller,
        id: &BrainProposalId,
    ) -> BrainResult<()> {
        self.decide(caller, id, ProposalStatus::Approved).await
    }

    pub async fn reject_proposal(
        &self,
        caller: &BrainCaller,
        id: &BrainProposalId,
    ) -> BrainResult<()> {
        self.decide(caller, id, ProposalStatus::Rejected).await
    }

    async fn decide(
        &self,
        caller: &BrainCaller,
        id: &BrainProposalId,
        status: ProposalStatus,
    ) -> BrainResult<()> {
        authority::may_decide_proposals(caller)?;
        if !ProposalRepository::decide(self.repos.write_pool(), id, status, &caller.user_id).await?
        {
            return Err(BrainError::Forbidden {
                reason: "the proposal is not open".to_owned(),
            });
        }
        tracing::info!(proposal_id = %id, actor = %caller.user_id, ?status, "Brain proposal decided");
        Ok(())
    }
}
