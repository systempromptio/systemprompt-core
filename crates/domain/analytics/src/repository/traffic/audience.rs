//! Geography, device, and bot traffic breakdowns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TrafficAnalyticsRepository;
use crate::Result;
use crate::models::reporting::{BotTotalsRow, BotTypeRow, DeviceRow, GeoRow};
use chrono::{DateTime, Utc};

impl TrafficAnalyticsRepository {
    pub async fn get_geo_breakdown(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        engaged_only: bool,
    ) -> Result<Vec<GeoRow>> {
        if engaged_only {
            sqlx::query_as!(
                GeoRow,
                r#"
                SELECT
                    COALESCE(country, 'Unknown') as "country",
                    COUNT(*)::bigint as "count!"
                FROM v_engaged_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY country
                ORDER BY COUNT(*) DESC
                LIMIT $3
                "#,
                start,
                end,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                GeoRow,
                r#"
                SELECT
                    COALESCE(country, 'Unknown') as "country",
                    COUNT(*)::bigint as "count!"
                FROM v_clean_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY country
                ORDER BY COUNT(*) DESC
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
    }

    pub async fn get_device_breakdown(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        engaged_only: bool,
    ) -> Result<Vec<DeviceRow>> {
        if engaged_only {
            sqlx::query_as!(
                DeviceRow,
                r#"
                SELECT
                    COALESCE(device_type, 'unknown') as "device",
                    COALESCE(browser, 'unknown') as "browser",
                    COUNT(*)::bigint as "count!"
                FROM v_engaged_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY device_type, browser
                ORDER BY COUNT(*) DESC
                LIMIT $3
                "#,
                start,
                end,
                limit
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                DeviceRow,
                r#"
                SELECT
                    COALESCE(device_type, 'unknown') as "device",
                    COALESCE(browser, 'unknown') as "browser",
                    COUNT(*)::bigint as "count!"
                FROM v_clean_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY device_type, browser
                ORDER BY COUNT(*) DESC
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
    }

    pub async fn get_bot_totals(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<BotTotalsRow> {
        sqlx::query_as!(
            BotTotalsRow,
            r#"
            SELECT
                COUNT(*) FILTER (WHERE is_bot = false AND is_ai_crawler = false AND is_scanner = false AND is_behavioral_bot = false AND landing_page IS NOT NULL AND request_count > 0)::bigint as "human!",
                COUNT(*) FILTER (WHERE is_bot = false AND is_ai_crawler = false AND is_scanner = false AND is_behavioral_bot = false AND (landing_page IS NULL OR request_count = 0))::bigint as "ghost!",
                COUNT(*) FILTER (WHERE is_bot = true OR is_ai_crawler = true OR is_scanner = true OR is_behavioral_bot = true)::bigint as "bot!"
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

    pub async fn get_bot_breakdown(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<BotTypeRow>> {
        sqlx::query_as!(
            BotTypeRow,
            r#"
            SELECT
                bot_type as "bot_type",
                COUNT(*)::bigint as "count!"
            FROM v_bot_sessions
            WHERE started_at >= $1 AND started_at < $2
            GROUP BY 1
            ORDER BY COUNT(*) DESC
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
