//! `governance_decisions` insert primitive.
//!
//! Single canonical writer for the table. Both the extension's
//! `POST /govern/authz` handler (for resolved decisions) and core's
//! [`DbAuditSink`](super::DbAuditSink) (for webhook-fault, default-deny, and
//! unrestricted-allow decisions) call this repository so there is exactly one
//! SQL statement that knows the column layout.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::Actor;

use crate::authz::types::DecisionTag;
use crate::policy::types::AccessScope;

pub const AUDIT_WRITE_FAILED_TOTAL: &str = "governance_audit_write_failed_total";

#[derive(Debug)]
pub struct GovernanceDecisionRecord<'a> {
    pub id: &'a str,
    pub actor: &'a Actor,
    pub session_id: &'a str,
    pub tool_name: &'a str,
    pub agent_id: Option<&'a str>,
    pub agent_scope: Option<AccessScope>,
    pub decision: DecisionTag,
    pub policy: &'a str,
    pub reason: &'a str,
    // JSON: governance audit blob — typed `DecisionAudit` on the writing side;
    // payload shape is documented in CHANGELOG and rendered by the dashboard.
    pub evaluated_rules: &'a serde_json::Value,
    pub plugin_id: Option<&'a str>,
    pub act_chain: &'a [Actor],
    pub context_id: &'a str,
    pub task_id: Option<&'a str>,
    // Why: the request-plane correlator gets its own field so the trace join
    // never depends on `session_id` carrying it.
    pub trace_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct GovernanceDecisionRepository {
    pool: std::sync::Arc<PgPool>,
}

impl GovernanceDecisionRepository {
    pub const fn from_pool(pool: std::sync::Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn insert(&self, record: &GovernanceDecisionRecord<'_>) -> Result<(), sqlx::Error> {
        insert_governance_decision(&self.pool, record).await
    }
}

/// One decision row as a client reads it back, keyed by the call id the gateway
/// returned to that client.
#[derive(Debug, Clone)]
pub struct GovernanceDecisionRow {
    pub call_id: String,
    pub decision: String,
    pub policy: String,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Why: `call_id` is not a column -- it lives inside the `evaluated_rules` audit
// blob -- so the filter is on the indexed `user_id`/`created_at` pair and the
// id is projected out. Reversing that would sequential-scan the whole table.
pub async fn recent_decisions_for_user(
    pool: &PgPool,
    user_id: &str,
    since: chrono::DateTime<chrono::Utc>,
    limit: i64,
) -> Result<Vec<GovernanceDecisionRow>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT evaluated_rules->>'call_id' AS call_id, decision, policy, reason, created_at \
         FROM governance_decisions WHERE user_id = $1 AND created_at >= $2 \
         AND evaluated_rules->>'call_id' IS NOT NULL ORDER BY created_at DESC LIMIT $3",
        user_id,
        since,
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(GovernanceDecisionRow {
                call_id: r.call_id?,
                decision: r.decision,
                policy: r.policy,
                reason: r.reason,
                created_at: r.created_at,
            })
        })
        .collect())
}

pub async fn insert_governance_decision(
    pool: &PgPool,
    record: &GovernanceDecisionRecord<'_>,
) -> Result<(), sqlx::Error> {
    let actor_kind = record.actor.kind.tag();
    let actor_id = record.actor.kind.actor_id(&record.actor.user_id);
    let act_chain =
        serde_json::to_value(record.act_chain).unwrap_or_else(|_| serde_json::json!([]));
    let result = sqlx::query!(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, agent_id, \
         agent_scope, decision, policy, reason, evaluated_rules, plugin_id, actor_kind, actor_id, \
         act_chain, context_id, task_id, trace_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
         $10, $11, $12, $13, $14, $15, $16, $17)",
        record.id,
        record.actor.user_id.as_str(),
        record.session_id,
        record.tool_name,
        record.agent_id,
        record.agent_scope.map(AccessScope::as_str),
        record.decision.as_str(),
        record.policy,
        record.reason,
        record.evaluated_rules,
        record.plugin_id,
        actor_kind.as_str(),
        actor_id,
        act_chain,
        record.context_id,
        record.task_id,
        record.trace_id,
    )
    .execute(pool)
    .await;
    if let Err(error) = &result {
        tracing::error!(
            error = %error,
            actor_kind = actor_kind.as_str(),
            actor_id,
            policy = record.policy,
            decision = record.decision.as_str(),
            session_id = record.session_id,
            "governance_decisions insert failed; audit row dropped"
        );
        metrics::counter!(
            AUDIT_WRITE_FAILED_TOTAL,
            "actor_kind" => actor_kind.as_str(),
            "decision" => record.decision.as_str(),
            "policy" => record.policy.to_owned(),
        )
        .increment(1);
    }
    result.map(|_| ())
}
