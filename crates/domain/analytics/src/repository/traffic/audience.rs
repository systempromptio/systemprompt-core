//! Geography, device, and bot traffic breakdowns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TrafficAnalyticsRepository;
use crate::models::reporting::{BotTotalsRow, BotTypeRow, DeviceRow, GeoRow};
use crate::{AnalyticsError, Result};
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
                FROM report_engaged_traffic
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
                FROM report_clean_traffic
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
                FROM report_engaged_traffic
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
                FROM report_clean_traffic
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
        let totals = self
            .traffic_counts
            .session_totals(start, end)
            .await
            .map_err(AnalyticsError::from)?;
        Ok(BotTotalsRow {
            human: totals.human,
            ghost: totals.ghost,
            bot: totals.bot,
        })
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
            FROM report_bot_sessions
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
