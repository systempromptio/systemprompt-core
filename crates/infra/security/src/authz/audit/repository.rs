//! `governance_decisions` insert primitive.
//!
//! Single canonical writer for the table. Both the extension's
//! `POST /govern/authz` handler (for resolved decisions) and core's
//! [`DbAuditSink`](super::DbAuditSink) (for webhook-fault, default-deny, and
//! unrestricted-allow decisions) call this repository so there is exactly one
//! SQL statement that knows the column layout.

use sqlx::PgPool;

#[derive(Debug)]
pub struct GovernanceDecisionRecord<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub session_id: &'a str,
    pub tool_name: &'a str,
    pub agent_id: Option<&'a str>,
    pub agent_scope: &'a str,
    pub decision: &'a str,
    pub policy: &'a str,
    pub reason: &'a str,
    pub evaluated_rules: &'a serde_json::Value,
    pub plugin_id: Option<&'a str>,
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
    sqlx::query!(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, agent_id, \
         agent_scope, decision, policy, reason, evaluated_rules, plugin_id) VALUES ($1, $2, $3, \
         $4, $5, $6, $7, $8, $9, $10, $11)",
        record.id,
        record.user_id,
        record.session_id,
        record.tool_name,
        record.agent_id,
        record.agent_scope,
        record.decision,
        record.policy,
        record.reason,
        record.evaluated_rules,
        record.plugin_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}
