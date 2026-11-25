//! Log-row insert and delete mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{LogId, UserId};

use crate::models::{LogEntry, LoggingError};

pub(in crate::repository) async fn create_log(
    pool: &PgPool,
    entry: &LogEntry,
) -> Result<(), LoggingError> {
    let metadata_json = entry
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    let level_str = entry.level.to_string();

    sqlx::query!(
        r"
        INSERT INTO logs (id, timestamp, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id, instance_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ",
        entry.id as _,
        entry.timestamp,
        level_str,
        entry.module,
        entry.message,
        metadata_json,
        entry.user_id as _,
        entry.session_id as _,
        entry.task_id as _,
        entry.trace_id as _,
        entry.context_id as _,
        entry.client_id as _,
        entry.instance_id as _
    )
    .execute(pool)
    .await
    ?;

    Ok(())
}

pub(in crate::repository) async fn update_log(
    pool: &PgPool,
    id: &LogId,
    entry: &LogEntry,
) -> Result<bool, LoggingError> {
    let metadata_json = entry
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    let level_str = entry.level.to_string();

    let result = sqlx::query!(
        r"
        UPDATE logs
        SET level = $1, module = $2, message = $3, metadata = $4
        WHERE id = $5
        ",
        level_str,
        entry.module,
        entry.message,
        metadata_json,
        id as _
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub(in crate::repository) async fn delete_log(
    pool: &PgPool,
    id: &LogId,
) -> Result<bool, LoggingError> {
    let result = sqlx::query!("DELETE FROM logs WHERE id = $1", id as _)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub(in crate::repository) async fn delete_logs_multiple(
    pool: &PgPool,
    ids: &[LogId],
) -> Result<u64, LoggingError> {
    if ids.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query!("DELETE FROM logs WHERE id = ANY($1)", ids as _)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub(in crate::repository) async fn clear_all_logs(pool: &PgPool) -> Result<u64, LoggingError> {
    let result = sqlx::query!("DELETE FROM logs").execute(pool).await?;

    Ok(result.rows_affected())
}

pub(in crate::repository) async fn purge_logs_before(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
    batch: i64,
) -> Result<u64, LoggingError> {
    let result = sqlx::query!(
        "DELETE FROM logs WHERE ctid = ANY(ARRAY(SELECT ctid FROM logs WHERE timestamp < $1 LIMIT $2))",
        cutoff,
        batch
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub(in crate::repository) async fn count_logs_before(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
) -> Result<u64, LoggingError> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM logs WHERE timestamp < $1"#,
        cutoff
    )
    .fetch_one(pool)
    .await?;

    Ok(count as u64)
}

pub(in crate::repository) async fn distinct_log_user_ids(
    pool: &PgPool,
) -> Result<Vec<UserId>, LoggingError> {
    let ids = sqlx::query_scalar!(
        r#"SELECT DISTINCT user_id AS "user_id!" FROM logs WHERE user_id IS NOT NULL"#
    )
    .fetch_all(pool)
    .await?;
    Ok(ids.into_iter().map(UserId::new).collect())
}

pub(in crate::repository) async fn delete_logs_for_users(
    pool: &PgPool,
    user_ids: &[UserId],
) -> Result<u64, LoggingError> {
    let result = sqlx::query!("DELETE FROM logs WHERE user_id = ANY($1)", user_ids as _)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub(in crate::repository) async fn count_logs_for_users(
    pool: &PgPool,
    user_ids: &[UserId],
) -> Result<u64, LoggingError> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM logs WHERE user_id = ANY($1)"#,
        user_ids as _
    )
    .fetch_one(pool)
    .await?;
    Ok(u64::try_from(count).unwrap_or(0))
}
