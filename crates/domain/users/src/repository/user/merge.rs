//! Anonymous-to-identified user merge operations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, Postgres, Transaction};
use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::repository::UserRepository;

#[derive(Debug, Clone, Copy)]
pub struct MergeResult {
    pub sessions: u64,
    pub tasks: u64,
    pub total_rows: u64,
}

// Why: `merge_users` transfers data rows only. Rows in these credential and
// auth-state tables are bound to the source identity and die with it via FK
// CASCADE when the source row is deleted. A new table holding credentials or
// auth state belongs on this list, not in a `transfer_*` helper.
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
        "UPDATE governance_decisions SET user_id = $1 WHERE user_id = $2",
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
