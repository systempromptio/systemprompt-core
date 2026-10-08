//! Withdrawal proposal persistence for Git synchronization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ManagedResourceId, SourceSnapshotId, UserId, WithdrawalProposalId};

use super::{WithdrawalProposal, WithdrawalStatus};
use crate::managed::{ManagedError, ManagedRepository, Result};

impl ManagedRepository {
    pub async fn list_withdrawal_proposals(
        &self,
        owner: &UserId,
    ) -> Result<Vec<WithdrawalProposal>> {
        Ok(sqlx::query_as!(WithdrawalProposal, r#"SELECT id AS "id: WithdrawalProposalId",resource_id AS "resource_id: ManagedResourceId",snapshot_id AS "snapshot_id: SourceSnapshotId",reason,status AS "status: WithdrawalStatus",created_at,decided_by AS "decided_by?: UserId",decided_at FROM managed_withdrawal_proposals WHERE owner_id=$1 ORDER BY created_at DESC LIMIT 100"#,
            owner.as_str()).fetch_all(&self.pool).await?)
    }

    pub async fn decide_withdrawal_proposal(
        &self,
        owner: &UserId,
        actor: &UserId,
        proposal: &WithdrawalProposalId,
        approved: bool,
    ) -> Result<()> {
        let status = if approved {
            WithdrawalStatus::Approved
        } else {
            WithdrawalStatus::Rejected
        };
        let changed = sqlx::query!("UPDATE managed_withdrawal_proposals SET status=$4,decided_by=$2,decided_at=NOW() WHERE id=$1 AND owner_id=$3 AND status='pending'",
            proposal.as_str(), actor.as_str(), owner.as_str(), status.as_str()).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Conflict(
                "Withdrawal proposal is stale or unavailable".to_owned(),
            ));
        }
        Ok(())
    }

    pub(super) async fn propose_withdrawal(
        &self,
        owner: &UserId,
        resource_id: &ManagedResourceId,
        snapshot_id: &SourceSnapshotId,
    ) -> Result<WithdrawalProposalId> {
        let proposal_id = WithdrawalProposalId::generate();
        sqlx::query!("INSERT INTO managed_withdrawal_proposals(id,owner_id,resource_id,snapshot_id,reason) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,resource_id,snapshot_id) DO NOTHING",
            proposal_id.as_str(), owner.as_str(), resource_id.as_str(), snapshot_id.as_str(), "Upstream Git tree removed the managed resource").execute(&self.pool).await?;
        let stored = sqlx::query_scalar!("SELECT id FROM managed_withdrawal_proposals WHERE owner_id=$1 AND resource_id=$2 AND snapshot_id=$3",
            owner.as_str(), resource_id.as_str(), snapshot_id.as_str()).fetch_one(&self.pool).await?;
        Ok(WithdrawalProposalId::new(stored))
    }
}
