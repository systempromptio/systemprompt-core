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
            SELECT
                COUNT(*)::bigint as "total_views!",
                COUNT(DISTINCT ae.session_id)::bigint as "unique_visitors!",
                COALESCE(AVG(ee.time_on_page_ms) / 1000.0, 0)::float8 as "avg_time_on_page_seconds",
                COALESCE(AVG(ee.max_scroll_depth), 0)::float8 as "avg_scroll_depth",
                COALESCE(SUM(ee.click_count), 0)::bigint as "total_clicks!"
            FROM analytics_events ae
            LEFT JOIN engagement_events ee ON ae.session_id = ee.session_id
            WHERE ae.event_type = 'page_view'
                AND ae.timestamp >= $1 AND ae.timestamp < $2
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
