use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::models::cli::{LiveSessionRow, SessionStatsRow, SessionTrendRow};

#[derive(Debug)]
pub struct CliSessionAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl CliSessionAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<SessionStatsRow> {
        sqlx::query_as!(
            SessionStatsRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_sessions!",
                COUNT(DISTINCT user_id)::bigint as "unique_users!",
                AVG(duration_seconds)::float8 as "avg_duration",
                AVG(request_count)::float8 as "avg_requests",
                COUNT(*) FILTER (WHERE converted_at IS NOT NULL)::bigint as "conversions!"
            FROM user_sessions
            WHERE started_at >= $1 AND started_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_active_session_count(&self, since: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM user_sessions WHERE ended_at IS NULL AND last_activity_at >= $1"#,
            since
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_live_sessions(
        &self,
        cutoff: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<LiveSessionRow>> {
        sqlx::query_as!(
            LiveSessionRow,
            r#"
            SELECT
                session_id as "session_id!",
                COALESCE(user_type, 'unknown') as "user_type",
                started_at as "started_at!",
                duration_seconds,
                request_count,
                last_activity_at as "last_activity_at!"
            FROM user_sessions
            WHERE ended_at IS NULL
              AND last_activity_at >= $1
            ORDER BY last_activity_at DESC
            LIMIT $2
            "#,
            cutoff,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_active_count(&self, cutoff: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM user_sessions WHERE ended_at IS NULL AND last_activity_at >= $1"#,
            cutoff
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_sessions_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SessionTrendRow>> {
        sqlx::query_as!(
            SessionTrendRow,
            r#"
            SELECT
                started_at as "started_at!",
                user_id as "user_id: UserId",
                duration_seconds
            FROM user_sessions
            WHERE started_at >= $1 AND started_at < $2
            ORDER BY started_at
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_active_count_since(&self, start: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::bigint as "count!"
            FROM user_sessions
            WHERE ended_at IS NULL
              AND last_activity_at >= $1
            "#,
            start
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_total_count(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM user_sessions WHERE started_at >= $1 AND started_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }
}
