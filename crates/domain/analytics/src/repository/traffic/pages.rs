//! Page and navigation traffic queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{NavigationQuery, PageQuery, TrafficAnalyticsRepository};
use crate::Result;
use crate::models::reporting::{TrafficNavigationRow, TrafficPageRow};

impl TrafficAnalyticsRepository {
    pub async fn get_pages(&self, query: PageQuery<'_>) -> Result<Vec<TrafficPageRow>> {
        let PageQuery {
            start,
            end,
            limit,
            engaged_only,
            referrer,
            path_prefix,
        } = query;
        if engaged_only {
            sqlx::query_as!(
                TrafficPageRow,
                r#"
                SELECT
                    landing_page as "page",
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM report_engaged_traffic
                WHERE started_at >= $1 AND started_at < $2
                  AND ($3::text IS NULL OR COALESCE(referrer_source, 'direct') = $3)
                  AND ($4::text IS NULL OR landing_page LIKE $4 || '%')
                GROUP BY landing_page, referrer_source
                ORDER BY COUNT(*) DESC
                LIMIT $5
                "#,
                start,
                end,
                referrer,
                path_prefix,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                TrafficPageRow,
                r#"
                SELECT
                    landing_page as "page",
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM report_clean_traffic
                WHERE started_at >= $1 AND started_at < $2
                  AND landing_page IS NOT NULL
                  AND ($3::text IS NULL OR COALESCE(referrer_source, 'direct') = $3)
                  AND ($4::text IS NULL OR landing_page LIKE $4 || '%')
                GROUP BY landing_page, referrer_source
                ORDER BY COUNT(*) DESC
                LIMIT $5
                "#,
                start,
                end,
                referrer,
                path_prefix,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn get_navigation(
        &self,
        query: NavigationQuery<'_>,
    ) -> Result<Vec<TrafficNavigationRow>> {
        let NavigationQuery {
            start,
            end,
            limit,
            path_prefix,
            internal_only,
        } = query;
        if internal_only {
            sqlx::query_as!(
                TrafficNavigationRow,
                r#"
                SELECT
                    NULLIF(endpoint, '') as "from_path",
                    NULLIF(target_url, '') as "to_path",
                    SUM(clicks)::bigint as "count!"
                FROM report_link_click_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND ($3::text IS NULL OR target_url LIKE $3 || '%')
                  AND NOT is_external
                GROUP BY endpoint, target_url
                ORDER BY SUM(clicks) DESC
                LIMIT $4
                "#,
                start,
                end,
                path_prefix,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                TrafficNavigationRow,
                r#"
                SELECT
                    NULLIF(endpoint, '') as "from_path",
                    NULLIF(target_url, '') as "to_path",
                    SUM(clicks)::bigint as "count!"
                FROM report_link_click_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND ($3::text IS NULL OR target_url LIKE $3 || '%')
                GROUP BY endpoint, target_url
                ORDER BY SUM(clicks) DESC
                LIMIT $4
                "#,
                start,
                end,
                path_prefix,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }
}
