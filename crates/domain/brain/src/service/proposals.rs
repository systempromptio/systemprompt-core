//! Proposals drafted by analyses: listed for review and approved or
//! rejected by an admin. Deciding a proposal only records the decision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::AnalysisWindow;
use systemprompt_identifiers::BrainProposalId;
use systemprompt_models::brain::{BrainProposal, ProposalStatus};

use super::{BrainCaller, BrainService, authority};
use crate::error::{BrainError, BrainResult};
use crate::pipeline::BatchContext;
use crate::replay::ReplayReport;
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

    pub async fn replay_proposal(
        &self,
        caller: &BrainCaller,
        id: &BrainProposalId,
        window: AnalysisWindow,
    ) -> BrainResult<ReplayReport> {
        authority::may_decide_proposals(caller)?;
        let Some(replay) = self.replay.get() else {
            return Err(BrainError::Forbidden {
                reason: "replay is not configured".to_owned(),
            });
        };
        if !ProposalRepository::exists(self.repos.write_pool(), id).await? {
            return Err(BrainError::ProposalNotFound(id.clone()));
        }
        let version = {
            let mut conn = self.repos.write_pool().acquire().await?;
            BatchContext::load(&mut conn).await?.version().clone()
        };
        let report = replay
            .service
            .dry_run(window, version, std::sync::Arc::clone(&replay.model))
            .await?;
        // JSON: the proposal's replay column stores the typed report.
        let stored = serde_json::to_value(&report)?;
        if !ProposalRepository::attach_replay(self.repos.write_pool(), id, &stored).await? {
            return Err(BrainError::ProposalNotFound(id.clone()));
        }
        tracing::info!(proposal_id = %id, actor = %caller.user_id, events = report.events, "Brain proposal replayed");
        Ok(report)
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
            if !ProposalRepository::exists(self.repos.write_pool(), id).await? {
                return Err(BrainError::ProposalNotFound(id.clone()));
            }
            return Err(BrainError::Forbidden {
                reason: "the proposal is not open".to_owned(),
            });
        }
        tracing::info!(proposal_id = %id, actor = %caller.user_id, ?status, "Brain proposal decided");
        Ok(())
    }
}
