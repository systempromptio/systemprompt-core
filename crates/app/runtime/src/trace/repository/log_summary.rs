//! Log summary over the `log_rollup_hourly` counters: level counts, module
//! counts, time range and total. A `since` selects whole UTC hours, from the
//! hour it falls in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use super::{Result, TraceError, TraceRepository};
use crate::trace::models::{LevelCount, LogTimeRange, ModuleCount};

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

impl TraceRepository {
    pub async fn count_logs_by_level(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<LevelCount>> {
        let rows = sqlx::query_as!(
            LevelRow,
            r#"
        SELECT level as "level!", SUM(log_count)::BIGINT as "count"
        FROM log_rollup_hourly
        WHERE ($1::TIMESTAMPTZ IS NULL OR bucket >= date_trunc('hour', $1, 'UTC'))
        GROUP BY level
        "#,
            since
        )
        .fetch_all(&*self.pool)
        .await?;

        rows.into_iter()
            .map(|r| -> Result<LevelCount> {
                Ok(LevelCount {
                    level: r.level.parse()?,
                    count: r.count.unwrap_or(0),
                })
            })
            .collect()
    }

    pub async fn top_modules(
        &self,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<ModuleCount>> {
        let rows = sqlx::query_as!(
            ModuleRow,
            r#"
        SELECT module as "module!", SUM(log_count)::BIGINT as "count"
        FROM log_rollup_hourly
        WHERE ($1::TIMESTAMPTZ IS NULL OR bucket >= date_trunc('hour', $1, 'UTC'))
        GROUP BY module
        ORDER BY count DESC
        LIMIT $2
        "#,
            since,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ModuleCount {
                module: r.module,
                count: r.count.unwrap_or(0),
            })
            .collect())
    }

    pub async fn log_time_range(&self, since: Option<DateTime<Utc>>) -> Result<LogTimeRange> {
        let row = sqlx::query_as!(
            TimeRangeRow,
            r#"
        SELECT MIN(first_at) as "earliest", MAX(last_at) as "latest"
        FROM log_rollup_hourly
        WHERE ($1::TIMESTAMPTZ IS NULL OR bucket >= date_trunc('hour', $1, 'UTC'))
        "#,
            since
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(LogTimeRange {
            earliest: row.earliest,
            latest: row.latest,
        })
    }

    pub async fn total_log_count(&self) -> Result<i64> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(log_count), 0)::BIGINT as "count!" FROM log_rollup_hourly"#
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(TraceError::from)
    }
}
