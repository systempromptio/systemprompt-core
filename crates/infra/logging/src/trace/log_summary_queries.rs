//! Log summary aggregates: level counts, module counts, time range.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::LoggingError;
pub(super) type Result<T> = std::result::Result<T, LoggingError>;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{LevelCount, LogTimeRange, ModuleCount};

struct LevelRow {
    level: String,
    count: Option<i64>,
}

struct ModuleRow {
    module: String,
    count: Option<i64>,
}

struct TimeRangeRow {
    earliest: Option<DateTime<Utc>>,
    latest: Option<DateTime<Utc>>,
}

pub(super) async fn count_logs_by_level(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LevelCount>> {
    let rows = sqlx::query_as!(
        LevelRow,
        r#"
        SELECT level as "level!", COUNT(*) as "count"
        FROM logs
        WHERE ($1::TIMESTAMPTZ IS NULL OR timestamp >= $1)
        GROUP BY level
        "#,
        since
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LevelCount {
            level: r.level,
            count: r.count.unwrap_or(0),
        })
        .collect())
}

pub(super) async fn top_modules(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<ModuleCount>> {
    let rows = sqlx::query_as!(
        ModuleRow,
        r#"
        SELECT module as "module!", COUNT(*) as "count"
        FROM logs
        WHERE ($1::TIMESTAMPTZ IS NULL OR timestamp >= $1)
        GROUP BY module
        ORDER BY count DESC
        LIMIT $2
        "#,
        since,
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ModuleCount {
            module: r.module,
            count: r.count.unwrap_or(0),
        })
        .collect())
}

pub(super) async fn log_time_range(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<LogTimeRange> {
    let row = sqlx::query_as!(
        TimeRangeRow,
        r#"
        SELECT MIN(timestamp) as "earliest", MAX(timestamp) as "latest"
        FROM logs
        WHERE ($1::TIMESTAMPTZ IS NULL OR timestamp >= $1)
        "#,
        since
    )
    .fetch_one(&**pool)
    .await?;

    Ok(LogTimeRange {
        earliest: row.earliest,
        latest: row.latest,
    })
}

pub(super) async fn total_log_count(pool: &Arc<PgPool>) -> Result<i64> {
    sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM logs"#)
        .fetch_one(&**pool)
        .await
        .map_err(LoggingError::from)
}
