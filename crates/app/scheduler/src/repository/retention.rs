//! The retention job's per-table batched deletes (and the
//! `ai_request_payloads` body release), plus the dry-run counts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::postgres::PgQueryResult;
use systemprompt_database::DbPool;

use crate::error::{SchedulerError, SchedulerResult};

pub(crate) const BATCH_ROWS: i64 = 5000;

#[derive(Debug, Clone)]
pub(crate) struct RetentionRepository {
    write_pool: Arc<PgPool>,
}

impl RetentionRepository {
    pub(crate) fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub(crate) async fn delete_batch(
        &self,
        table: &str,
        cutoff: DateTime<Utc>,
    ) -> SchedulerResult<u64> {
        let result = match self.volume_batch(table, cutoff).await {
            Some(result) => result,
            None => match self.audit_batch(table, cutoff).await {
                Some(result) => result,
                None => {
                    return Err(SchedulerError::UnknownRetentionTable {
                        table: table.to_owned(),
                    });
                },
            },
        };
        Ok(result?.rows_affected())
    }

    // Why: every arm is a `sqlx::query!`, so the SQL is verified against the
    // schema at compile time and the table name cannot be interpolated into one
    // shared statement. The split is by table, not by behaviour.
    async fn volume_batch(
        &self,
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
                .execute(&*self.write_pool)
                .await
            },
            "analytics_events" => {
                sqlx::query!(
                    "DELETE FROM analytics_events WHERE ctid = ANY(ARRAY(SELECT ctid FROM analytics_events WHERE timestamp < $1 LIMIT $2))",
                    cutoff,
                    BATCH_ROWS
                )
                .execute(&*self.write_pool)
                .await
            },
            "ai_request_messages" => {
                sqlx::query!(
                    "DELETE FROM ai_request_messages WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_request_messages WHERE created_at < $1 LIMIT $2))",
                    cutoff,
                    BATCH_ROWS
                )
                .execute(&*self.write_pool)
                .await
            },
            "mcp_tool_executions" => {
                sqlx::query!(
                    "DELETE FROM mcp_tool_executions WHERE ctid = ANY(ARRAY(SELECT ctid FROM mcp_tool_executions WHERE created_at < $1 LIMIT $2))",
                    cutoff,
                    BATCH_ROWS
                )
                .execute(&*self.write_pool)
                .await
            },
            "event_outbox" => {
                sqlx::query!(
                    "DELETE FROM event_outbox WHERE ctid = ANY(ARRAY(SELECT ctid FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL) LIMIT $2))",
                    cutoff,
                    BATCH_ROWS
                )
                .execute(&*self.write_pool)
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
                .execute(&*self.write_pool)
                .await
            },
            _ => return None,
        };
        Some(result)
    }

    async fn audit_batch(
        &self,
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
                .execute(&*self.write_pool)
                .await
            },
            "ai_quota_buckets" => {
                sqlx::query!(
                    "DELETE FROM ai_quota_buckets WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_quota_buckets WHERE window_start < $1 LIMIT $2))",
                    cutoff,
                    BATCH_ROWS
                )
                .execute(&*self.write_pool)
                .await
            },
            _ => return None,
        };
        Some(result)
    }

    pub(crate) async fn count_before(
        &self,
        table: &str,
        cutoff: DateTime<Utc>,
    ) -> SchedulerResult<i64> {
        let count = match table {
            "logs" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM logs WHERE timestamp < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "analytics_events" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM analytics_events WHERE timestamp < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "ai_request_messages" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM ai_request_messages WHERE created_at < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "mcp_tool_executions" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM mcp_tool_executions WHERE created_at < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "event_outbox" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL)"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "ai_request_payloads" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM ai_request_payloads WHERE created_at < $1 AND (request_body IS NOT NULL OR response_body IS NOT NULL)"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "governance_decisions" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM governance_decisions WHERE created_at < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            "ai_quota_buckets" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!" FROM ai_quota_buckets WHERE window_start < $1"#,
                    cutoff
                )
                .fetch_one(&*self.write_pool)
                .await
            },
            other => {
                return Err(SchedulerError::UnknownRetentionTable {
                    table: other.to_owned(),
                });
            },
        };
        count.map_err(SchedulerError::from)
    }
}
