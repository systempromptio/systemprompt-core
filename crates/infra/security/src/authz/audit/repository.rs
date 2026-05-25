//! `governance_decisions` insert primitive.
//!
//! Single canonical writer for the table. Both the extension's
//! `POST /govern/authz` handler (for resolved decisions) and core's
//! [`DbAuditSink`](super::DbAuditSink) (for webhook-fault, default-deny, and
//! unrestricted-allow decisions) call this repository so there is exactly one
//! SQL statement that knows the column layout.

use sqlx::PgPool;
use systemprompt_identifiers::Actor;

use crate::authz::types::DecisionTag;
use crate::policy::types::AccessScope;

/// Prometheus counter incremented whenever a `governance_decisions` INSERT
/// fails.
///
/// Exposed as a `pub const` so alert rules and dashboards can reference the
/// metric by symbol rather than re-typing the literal.
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
    /// RFC 8693 delegation lineage in outermost-first order. Empty for
    /// direct (non-delegated) tokens.
    pub act_chain: &'a [Actor],
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
    // Why: act_chain is `Vec<Actor>` which is unconditionally serde-compliant,
    // so serialization failure is unreachable; falling back to `[]` keeps the
    // audit row writable rather than dropping the entire governance record.
    let act_chain =
        serde_json::to_value(record.act_chain).unwrap_or_else(|_| serde_json::json!([]));
    let result = sqlx::query!(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, agent_id, \
         agent_scope, decision, policy, reason, evaluated_rules, plugin_id, actor_kind, actor_id, \
         act_chain) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
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
    )
    .execute(pool)
    .await;
    if let Err(error) = &result {
        // Why: callers run this inside `tokio::spawn` (fire-and-forget audit
        // writes), so a swallowed error here is invisible to the HTTP
        // response. Log + counter at the SQL boundary guarantees every drop
        // surfaces regardless of caller.
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
