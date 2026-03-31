use anyhow::{Context, Result};
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

pub async fn count_logs_by_level(
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
    .await
    .context("Failed to count logs by level")?;

    Ok(rows
        .into_iter()
        .map(|r| LevelCount {
            level: r.level,
            count: r.count.unwrap_or(0),
        })
        .collect())
}

pub async fn top_modules(
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
    .await
    .context("Failed to get top modules")?;

    Ok(rows
        .into_iter()
        .map(|r| ModuleCount {
            module: r.module,
            count: r.count.unwrap_or(0),
        })
        .collect())
}

pub async fn log_time_range(
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
    .await
    .context("Failed to get log time range")?;

    Ok(LogTimeRange {
        earliest: row.earliest,
        latest: row.latest,
    })
}

pub async fn total_log_count(pool: &Arc<PgPool>) -> Result<i64> {
    sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM logs"#)
        .fetch_one(&**pool)
        .await
        .context("Failed to count total logs")
}
