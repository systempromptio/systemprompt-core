//! `governance_decisions` insert primitive and the warn-mode read side.
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
    pub client_id: Option<&'a str>,
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
         act_chain, context_id, task_id, trace_id, client_id) VALUES ($1, $2, $3, $4, $5, $6, \
         $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
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
        record.client_id,
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

/// One row of the warn-mode rollup: a policy that fired in `mode: warn` for
/// one tool and one user, with the most recent reason it gave.
#[derive(Debug, Clone)]
pub struct GovernanceWarningRow {
    pub policy: String,
    pub tool_name: String,
    pub user_id: String,
    pub count: i64,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub example_reason: String,
}

// Why: grouped by all three dimensions at once so a caller can re-aggregate to
// whichever one it wants without a second round trip. The combinations are
// bounded by the policy count times the tool count, not by traffic.
pub async fn list_governance_warnings(
    pool: &PgPool,
    since: Option<chrono::DateTime<chrono::Utc>>,
    limit: i64,
) -> Result<Vec<GovernanceWarningRow>, sqlx::Error> {
    sqlx::query_as!(
        GovernanceWarningRow,
        r#"
        SELECT policy AS "policy!", tool_name AS "tool_name!", user_id AS "user_id!",
               COUNT(*) AS "count!", MIN(created_at) AS "first_seen!",
               MAX(created_at) AS "last_seen!",
               (ARRAY_AGG(reason ORDER BY created_at DESC))[1] AS "example_reason!"
        FROM governance_decisions
        WHERE decision = 'warn' AND ($1::timestamptz IS NULL OR created_at >= $1)
        GROUP BY policy, tool_name, user_id
        ORDER BY COUNT(*) DESC, MAX(created_at) DESC
        LIMIT $2
        "#,
        since,
        limit
    )
    .fetch_all(pool)
    .await
}

// Why: kept separate from the trace listing query so `trace list --decision`
// filters an existing result set rather than reshaping the trace query, which
// already unions four tables for every listing.
pub async fn list_trace_ids_with_decision(
    pool: &PgPool,
    decision: &str,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT DISTINCT trace_id AS "trace_id!"
        FROM governance_decisions
        WHERE decision = $1 AND trace_id IS NOT NULL
          AND ($2::timestamptz IS NULL OR created_at >= $2)
        "#,
        decision,
        since
    )
    .fetch_all(pool)
    .await
}
