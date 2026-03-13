use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::models::cli::{ContentStatsRow, ContentTrendRow, TopContentRow};

#[derive(Debug)]
pub struct ContentAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl ContentAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_top_content(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<TopContentRow>> {
        sqlx::query_as!(
            TopContentRow,
            r#"
            WITH content_stats AS (
                SELECT
                    ee.content_id,
                    COUNT(*)::bigint as total_views,
                    COUNT(DISTINCT ee.session_id)::bigint as unique_visitors,
                    (AVG(LEAST(ee.time_on_page_ms, 1800000)) / 1000.0)::float8 as avg_time_on_page_seconds
                FROM engagement_events ee
                INNER JOIN user_sessions us ON ee.session_id = us.session_id
                WHERE ee.created_at >= $1 AND ee.created_at < $2
                    AND ee.content_id IS NOT NULL
                    AND us.is_bot = false AND us.is_behavioral_bot = false AND us.is_scanner = false
                GROUP BY ee.content_id
            )
            SELECT
                cs.content_id as "content_id!",
                mc.slug as "slug?",
                mc.title as "title?",
                mc.source_id as "source_id?",
                cs.total_views as "total_views!",
                cs.unique_visitors as "unique_visitors!",
                cs.avg_time_on_page_seconds::float8 as "avg_time_on_page_seconds",
                NULL::text as "trend_direction"
            FROM content_stats cs
            LEFT JOIN markdown_content mc ON cs.content_id = mc.id
            ORDER BY cs.total_views DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ContentStatsRow> {
        sqlx::query_as!(
            ContentStatsRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_views!",
                COUNT(DISTINCT ee.session_id)::bigint as "unique_visitors!",
                COALESCE(AVG(LEAST(ee.time_on_page_ms, 1800000)) / 1000.0, 0)::float8 as "avg_time_on_page_seconds",
                COALESCE(AVG(ee.max_scroll_depth), 0)::float8 as "avg_scroll_depth",
                COALESCE(SUM(ee.click_count), 0)::bigint as "total_clicks!"
            FROM engagement_events ee
            INNER JOIN user_sessions us ON ee.session_id = us.session_id
            WHERE ee.created_at >= $1 AND ee.created_at < $2
                AND us.is_bot = false AND us.is_behavioral_bot = false AND us.is_scanner = false
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_content_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ContentTrendRow>> {
        sqlx::query_as!(
            ContentTrendRow,
            r#"
            WITH date_series AS (
                SELECT generate_series(
                    date_trunc('day', $1::timestamptz),
                    date_trunc('day', $2::timestamptz) - interval '1 day',
                    '1 day'::interval
                ) as day
            ),
            daily_stats AS (
                SELECT
                    date_trunc('day', ee.created_at) as day,
                    COUNT(*)::bigint as views,
                    COUNT(DISTINCT ee.session_id)::bigint as unique_visitors
                FROM engagement_events ee
                INNER JOIN user_sessions us ON ee.session_id = us.session_id
                WHERE ee.created_at >= $1 AND ee.created_at < $2
                    AND us.is_bot = false AND us.is_behavioral_bot = false AND us.is_scanner = false
                GROUP BY date_trunc('day', ee.created_at)
            )
            SELECT
                ds.day as "timestamp!",
                COALESCE(s.views, 0)::bigint as "views!",
                COALESCE(s.unique_visitors, 0)::bigint as "unique_visitors!"
            FROM date_series ds
            LEFT JOIN daily_stats s ON ds.day = s.day
            ORDER BY ds.day
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
