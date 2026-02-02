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
            SELECT
                content_id as "content_id!",
                total_views as "total_views!",
                unique_visitors as "unique_visitors!",
                avg_time_on_page_seconds as "avg_time_on_page_seconds",
                trend_direction as "trend_direction"
            FROM content_performance_metrics
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY total_views DESC
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
            WITH page_view_stats AS (
                SELECT
                    COUNT(*) as total_views,
                    COUNT(DISTINCT user_id) as unique_visitors
                FROM analytics_events
                WHERE event_type = 'page_view'
                    AND timestamp >= $1 AND timestamp < $2
            ),
            engagement_stats AS (
                SELECT
                    COALESCE(AVG(time_on_page_ms) / 1000.0, 0) as avg_time_on_page_seconds,
                    COALESCE(AVG(max_scroll_depth), 0) as avg_scroll_depth,
                    COALESCE(SUM(click_count), 0) as total_clicks
                FROM engagement_events
                WHERE created_at >= $1 AND created_at < $2
            )
            SELECT
                pv.total_views::bigint as "total_views!",
                pv.unique_visitors::bigint as "unique_visitors!",
                es.avg_time_on_page_seconds::float8 as "avg_time_on_page_seconds",
                es.avg_scroll_depth::float8 as "avg_scroll_depth",
                es.total_clicks::bigint as "total_clicks!"
            FROM page_view_stats pv, engagement_stats es
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
            SELECT
                created_at as "timestamp!",
                total_views as "views!",
                unique_visitors as "unique_visitors!"
            FROM content_performance_metrics
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY created_at
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
