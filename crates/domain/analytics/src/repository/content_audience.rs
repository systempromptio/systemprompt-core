//! Content audience aggregates over the analytics-owned engagement and event
//! views: per-content view totals, window statistics, the daily series and
//! the distinct human viewers per content endpoint. Content metadata is the
//! content member's and is joined by the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContentId;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::content::{ContentAudienceCounts, EndpointViewers};

use crate::Result;
use crate::models::reporting::{ContentStatsRow, ContentTrendRow};

#[derive(Debug, Clone, Copy)]
pub struct ContentViewsRow {
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ContentViewsEntry {
    pub content_id: ContentId,
    pub views: ContentViewsRow,
}

#[derive(Debug, Clone)]
pub struct ContentAudienceRepository {
    pool: Arc<PgPool>,
}

impl ContentAudienceRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }

    pub async fn top_content_views(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContentViewsEntry>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                ee.content_id as "content_id!: ContentId",
                COUNT(*)::bigint as "total_views!",
                COUNT(DISTINCT ee.session_id)::bigint as "unique_visitors!",
                (AVG(LEAST(ee.time_on_page_ms, 1800000)) / 1000.0)::float8 as "avg_time_on_page_seconds"
            FROM engagement_events ee
            INNER JOIN report_clean_traffic us ON ee.session_id = us.session_id
            WHERE ee.created_at >= $1 AND ee.created_at < $2
                AND ee.content_id IS NOT NULL
            GROUP BY ee.content_id
            ORDER BY COUNT(*) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ContentViewsEntry {
                content_id: row.content_id,
                views: ContentViewsRow {
                    total_views: row.total_views,
                    unique_visitors: row.unique_visitors,
                    avg_time_on_page_seconds: row.avg_time_on_page_seconds,
                },
            })
            .collect())
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
            INNER JOIN report_clean_traffic us ON ee.session_id = us.session_id
            WHERE ee.created_at >= $1 AND ee.created_at < $2
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
                INNER JOIN report_clean_traffic us ON ee.session_id = us.session_id
                WHERE ee.created_at >= $1 AND ee.created_at < $2
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

#[async_trait]
impl ContentAudienceCounts for ContentAudienceRepository {
    async fn endpoint_viewers(
        &self,
        endpoint_prefix: &str,
        days: i32,
    ) -> std::result::Result<Vec<EndpointViewers>, RepositoryError> {
        // Why: `users` carries only `is_bot` and `is_scanner`; this is a
        // user-level filter, not the session predicate `is_human_traffic`
        // states over the crawler and behavioural flags.
        let rows = sqlx::query!(
            r#"
            SELECT
                ae.endpoint as "endpoint!",
                COUNT(DISTINCT CASE
                    WHEN u.id IS NOT NULL AND u.is_bot = FALSE AND u.is_scanner = FALSE
                    THEN ae.user_id
                END)::bigint as "viewers!"
            FROM report_analytics_events ae
            LEFT JOIN report_users u ON ae.user_id = u.id
            WHERE ae.event_type = 'page_view'
                AND ae.event_category = 'content'
                AND left(ae.endpoint, length($1)) = $1
                AND ae.timestamp >= CURRENT_TIMESTAMP - ($2 || ' days')::INTERVAL
            GROUP BY ae.endpoint
            "#,
            endpoint_prefix,
            days.to_string()
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| EndpointViewers {
                endpoint: row.endpoint,
                viewers: row.viewers,
            })
            .collect())
    }
}
