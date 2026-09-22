//! The per-table batched deletes (and the `ai_request_payloads` body release)
//! that the retention plan is executed with, plus the dry-run counts.
//!
//! Each table is deleted in batches of `BATCH_ROWS` so no statement takes a
//! long lock and each statement-level capture trigger writes one outbox row
//! per batch instead of one per row. A run stops early at `RUN_BUDGET` and
//! picks up where it left off the next night.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::postgres::PgQueryResult;
use systemprompt_traits::{ProviderError, ProviderResult};

const BATCH_ROWS: i64 = 5000;
const RUN_BUDGET: Duration = Duration::from_mins(15);

#[derive(Debug, Clone)]
pub(super) struct RetentionPass {
    pub(super) table: &'static str,
    pub(super) days: u32,
    pub(super) deleted: u64,
    pub(super) complete: bool,
}

pub(super) async fn delete_in_batches(
    pool: &PgPool,
    table: &'static str,
    days: u32,
    cutoff: DateTime<Utc>,
    started: Instant,
) -> ProviderResult<RetentionPass> {
    let mut deleted = 0;
    loop {
        if started.elapsed() > RUN_BUDGET {
            return Ok(RetentionPass {
                table,
                days,
                deleted,
                complete: false,
            });
        }
        let batch = delete_batch(pool, table, cutoff).await?;
        deleted += batch;
        if batch < u64::try_from(BATCH_ROWS).unwrap_or(u64::MAX) {
            return Ok(RetentionPass {
                table,
                days,
                deleted,
                complete: true,
            });
        }
    }
}

pub(super) async fn delete_batch(
    pool: &PgPool,
    table: &str,
    cutoff: DateTime<Utc>,
) -> ProviderResult<u64> {
    let result = match volume_batch(pool, table, cutoff).await {
        Some(result) => result,
        None => match audit_batch(pool, table, cutoff).await {
            Some(result) => result,
            None => {
                return Err(ProviderError::Configuration(format!(
                    "no retention path for table {table}"
                )));
            },
        },
    };
    Ok(result
        .map_err(|e| ProviderError::Internal(e.to_string()))?
        .rows_affected())
}

// Why: every arm is a `sqlx::query!`, so the SQL is verified against the
// schema at compile time and the table name cannot be interpolated into one
// shared statement. The split is by table, not by behaviour.
async fn volume_batch(
    pool: &PgPool,
    table: &str,
    cutoff: DateTime<Utc>,
) -> Option<Result<PgQueryResult, sqlx::Error>> {
    let result = match table {

        "logs" => {
            sqlx::query!(
                "DELETE FROM logs WHERE ctid = ANY(ARRAY(SELECT ctid FROM logs WHERE timestamp < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "analytics_events" => {
            sqlx::query!(
                "DELETE FROM analytics_events WHERE ctid = ANY(ARRAY(SELECT ctid FROM analytics_events WHERE timestamp < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "ai_request_messages" => {
            sqlx::query!(
                "DELETE FROM ai_request_messages WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_request_messages WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "mcp_tool_executions" => {
            sqlx::query!(
                "DELETE FROM mcp_tool_executions WHERE ctid = ANY(ARRAY(SELECT ctid FROM mcp_tool_executions WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "event_outbox" => {
            sqlx::query!(
                "DELETE FROM event_outbox WHERE ctid = ANY(ARRAY(SELECT ctid FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL) LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        // Why: the payload row stays (excerpts, hashes, sizes are the audit
        // trail); only the raw bodies, the bulk of the table, are released.
        "ai_request_payloads" => {
            sqlx::query!(
                "UPDATE ai_request_payloads SET request_body = NULL, response_body = NULL, updated_at = NOW() \
                 WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_request_payloads WHERE created_at < $1 \
                 AND (request_body IS NOT NULL OR response_body IS NOT NULL) LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        _ => return None,
    };
    Some(result)
}

async fn audit_batch(
    pool: &PgPool,
    table: &str,
    cutoff: DateTime<Utc>,
) -> Option<Result<PgQueryResult, sqlx::Error>> {
    let result = match table {
        "governance_decisions" => {
            sqlx::query!(
                "DELETE FROM governance_decisions WHERE ctid = ANY(ARRAY(SELECT ctid FROM governance_decisions WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "ai_quota_buckets" => {
            sqlx::query!(
                "DELETE FROM ai_quota_buckets WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_quota_buckets WHERE window_start < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        _ => return None,
    };
    Some(result)
}

pub(super) async fn count_before(
    pool: &PgPool,
    table: &str,
    cutoff: DateTime<Utc>,
) -> ProviderResult<i64> {
    let count = match table {
        "logs" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM logs WHERE timestamp < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "analytics_events" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM analytics_events WHERE timestamp < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_request_messages" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_request_messages WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "mcp_tool_executions" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM mcp_tool_executions WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "event_outbox" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL)"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_request_payloads" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_request_payloads WHERE created_at < $1 AND (request_body IS NOT NULL OR response_body IS NOT NULL)"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "governance_decisions" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM governance_decisions WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_quota_buckets" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_quota_buckets WHERE window_start < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        other => {
            return Err(ProviderError::Configuration(format!(
                "no retention path for table {other}"
            )));
        },
    };
    count.map_err(|e| ProviderError::Internal(e.to_string()))
}
