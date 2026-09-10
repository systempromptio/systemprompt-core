//! Anonymous-to-identified user merge operations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, Postgres, Transaction};
use systemprompt_identifiers::{ContextId, SessionId, UserId};

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
    pub async fn merge_users(&self, source_id: &UserId, target_id: &UserId) -> Result<MergeResult> {
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;

        let source = source_id.as_str();
        let target = target_id.as_str();

        let sessions = transfer_sessions(&mut tx, source, target).await?;
        let tasks = transfer_tasks(&mut tx, source, target).await?;
        let mut total_rows = sessions + tasks;
        total_rows += transfer_audit_rows(&mut tx, source, target).await?;
        total_rows += transfer_content_rows(&mut tx, source, target).await?;
        record_merge_attribution(&mut tx, source, target).await?;

        sqlx::query!(
            "UPDATE fingerprint_reputation SET associated_user_ids = \
             array_replace(associated_user_ids, $2, $1) WHERE $2 = ANY(associated_user_ids)",
            target,
            source
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "DELETE FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1",
            source
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!("DELETE FROM users WHERE id = $1", source)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(MergeResult {
            sessions,
            tasks,
            total_rows,
        })
    }
}

async fn transfer_sessions(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    target: &str,
) -> Result<u64> {
    let result = sqlx::query!(
        "UPDATE user_sessions SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

async fn transfer_tasks(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    target: &str,
) -> Result<u64> {
    let result = sqlx::query!(
        "UPDATE agent_tasks SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

// Why: governance_decisions is append-only — a decision is evidence of what was
// authorised for whom at the time, so the merge is recorded as a new decision
// rather than by re-attributing the source user's history to the target. A
// reader following the target's trail finds this row and the source id in it.
async fn record_merge_attribution(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    target: &str,
) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let context_id = ContextId::derived_from_session(&SessionId::new(id.clone()));
    sqlx::query!(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, policy, \
         reason, actor_kind, actor_id, context_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
         $10)",
        id,
        target,
        id,
        MERGE_TOOL_NAME,
        "allow",
        MERGE_POLICY,
        format!("account merge: {source} merged into {target}"),
        "system",
        target,
        context_id.as_str(),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn transfer_audit_rows(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    target: &str,
) -> Result<u64> {
    let mut moved = 0;
    moved += sqlx::query!(
        "UPDATE task_messages SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE user_contexts SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE mcp_tool_executions SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE mcp_artifacts SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE mcp_sessions SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE logs SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok(moved)
}

async fn transfer_content_rows(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    target: &str,
) -> Result<u64> {
    let mut moved = 0;
    moved += sqlx::query!(
        "UPDATE ai_requests SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE engagement_events SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE analytics_events SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE event_outbox SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE files SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    moved += sqlx::query!(
        "UPDATE link_clicks SET user_id = $1 WHERE user_id = $2",
        target,
        source
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok(moved)
}
