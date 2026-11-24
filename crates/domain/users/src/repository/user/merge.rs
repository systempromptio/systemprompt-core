//! An account merge, in one transaction.
//!
//! Every admitted owner reassignment of the composition
//! ([`AdmittedOwnerReassignments`]) is rendered from its declaration and run
//! first; then this crate's own rows: the source's sessions move to the
//! target, the merge is recorded as a governance decision, and the source user
//! is deleted. Everything commits together, so a failure anywhere leaves every
//! row and the source user in place for a rerun. A reassignment naming a
//! table in `MERGE_EXCLUDED_SECURITY_TABLES` refuses the merge before any
//! row moves: those rows die with the source user.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, Postgres, Transaction};
use systemprompt_extension::AdmittedOwnerReassignments;
use systemprompt_governance::authz::types::DecisionTag;
use systemprompt_governance::authz::{GovernanceDecisionRecord, insert_governance_decision};
use systemprompt_identifiers::{Actor, ContextId, SessionId, UserId};
use systemprompt_traits::MERGE_EXCLUDED_SECURITY_TABLES;

use crate::error::{Result, UserError};
use crate::repository::UserRepository;

const MERGE_TOOL_NAME: &str = "users.merge";
const MERGE_POLICY: &str = "account_merge";

#[derive(Debug, Clone, Copy)]
pub struct MergeResult {
    pub sessions: u64,
    pub tasks: u64,
    pub total_rows: u64,
}

impl UserRepository {
    pub async fn merge(
        &self,
        source_id: &UserId,
        target_id: &UserId,
        reassignments: &AdmittedOwnerReassignments,
    ) -> Result<MergeResult> {
        if let Some(owned) = reassignments
            .entries()
            .iter()
            .find(|owned| MERGE_EXCLUDED_SECURITY_TABLES.contains(&owned.reassignment.table))
        {
            return Err(UserError::SecurityTableReassignment {
                owner: owned.owner.clone(),
                table: owned.reassignment.table,
            });
        }
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;
        Self::lock_unless_held(&mut tx, source_id).await?;

        let moved = Self::reassign_owned_rows(&mut tx, source_id, target_id, reassignments).await?;
        let tasks = moved
            .iter()
            .filter(|(table, _)| *table == "agent_tasks")
            .map(|(_, rows)| rows)
            .sum::<u64>();
        let reassigned = moved.iter().map(|(_, rows)| rows).sum::<u64>();

        let sessions = sqlx::query!(
            "UPDATE user_sessions SET user_id = $1 WHERE user_id = $2",
            target_id as _,
            source_id as _
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        record_merge_attribution(&mut tx, source_id, target_id).await?;

        sqlx::query!("DELETE FROM users WHERE id = $1", source_id as _)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(MergeResult {
            sessions,
            tasks,
            total_rows: reassigned + sessions,
        })
    }
}

// Why: governance_decisions is append-only — a decision is evidence of what was
// authorised for whom at the time, so the merge is recorded as a new decision
// rather than by re-attributing the source user's history to the target. A
// reader following the target's trail finds this row and the source id in it.
async fn record_merge_attribution(
    tx: &mut Transaction<'_, Postgres>,
    source_id: &UserId,
    target_id: &UserId,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let session_id = SessionId::new(id.clone());
    let context_id = ContextId::derived_from_session(&session_id);
    let actor = Actor::system(target_id.clone());
    let reason = format!("account merge: {source_id} merged into {target_id}");
    let evaluated_rules = serde_json::json!([]);
    let record = GovernanceDecisionRecord {
        id: &id,
        actor: &actor,
        session_id: Some(&session_id),
        tool_name: MERGE_TOOL_NAME,
        agent_id: None,
        agent_scope: None,
        decision: DecisionTag::Allow,
        policy: MERGE_POLICY,
        reason: &reason,
        evaluated_rules: &evaluated_rules,
        plugin_id: None,
        act_chain: &[],
        context_id: &context_id,
        task_id: None,
        trace_id: None,
        client_id: None,
        tool_use_id: None,
    };
    insert_governance_decision(&mut **tx, &record).await?;
    Ok(())
}
