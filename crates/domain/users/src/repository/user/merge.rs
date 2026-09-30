//! The users-owned half of an account merge.
//!
//! Every other domain's rows are moved by its own
//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) before this
//! runs; what is left is this crate's own: the source's sessions move to the
//! target, the merge is recorded as a governance decision, and the source user
//! is deleted — in one transaction, so a failure leaves the source in place
//! for a rerun.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, Postgres, Transaction};
use systemprompt_identifiers::{Actor, ContextId, SessionId, UserId};
use systemprompt_security::authz::types::DecisionTag;
use systemprompt_security::authz::{GovernanceDecisionRecord, insert_governance_decision};

use crate::error::Result;
use crate::repository::UserRepository;

const MERGE_TOOL_NAME: &str = "users.merge";
const MERGE_POLICY: &str = "account_merge";

#[derive(Debug, Clone, Copy)]
pub struct MergeResult {
    pub sessions: u64,
    pub tasks: u64,
    pub total_rows: u64,
}

pub const MERGE_EXCLUDED_SECURITY_TABLES: &[&str] = &[
    "oauth_auth_codes",
    "oauth_refresh_tokens",
    "oauth_clients",
    "webauthn_credentials",
    "webauthn_challenges",
    "webauthn_setup_tokens",
    "user_api_keys",
    "user_device_certs",
    "bridge_sessions",
    "bridge_exchange_codes",
    "federated_identities",
];

impl UserRepository {
    pub async fn complete_merge(&self, source_id: &UserId, target_id: &UserId) -> Result<u64> {
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;

        let sessions = sqlx::query!(
            "UPDATE user_sessions SET user_id = $1 WHERE user_id = $2",
            target_id.as_str(),
            source_id.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        record_merge_attribution(&mut tx, source_id, target_id).await?;

        sqlx::query!("DELETE FROM users WHERE id = $1", source_id.as_str())
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(sessions)
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
    let context_id = ContextId::derived_from_session(&SessionId::new(id.clone()));
    let actor = Actor::system(target_id.clone());
    let reason = format!("account merge: {source_id} merged into {target_id}");
    let evaluated_rules = serde_json::json!([]);
    let record = GovernanceDecisionRecord {
        id: &id,
        actor: &actor,
        session_id: &id,
        tool_name: MERGE_TOOL_NAME,
        agent_id: None,
        agent_scope: None,
        decision: DecisionTag::Allow,
        policy: MERGE_POLICY,
        reason: &reason,
        evaluated_rules: &evaluated_rules,
        plugin_id: None,
        act_chain: &[],
        context_id: context_id.as_str(),
        task_id: None,
        trace_id: None,
        client_id: None,
        tool_use_id: None,
    };
    insert_governance_decision(&mut **tx, &record).await?;
    Ok(())
}
