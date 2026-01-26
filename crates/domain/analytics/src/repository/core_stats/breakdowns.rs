use anyhow::Result;

use super::CoreStatsRepository;
use crate::models::{BotTrafficStats, BrowserBreakdown, DeviceBreakdown, GeographicBreakdown};

impl CoreStatsRepository {
    pub async fn get_browser_breakdown(&self, limit: i64) -> Result<Vec<BrowserBreakdown>> {
        sqlx::query_as!(
            BrowserBreakdown,
            r#"
            WITH browser_counts AS (
                SELECT
                    COALESCE(browser, 'Unknown') as browser,
                    COUNT(*) as count
                FROM user_sessions
                WHERE started_at >= NOW() - INTERVAL '7 days'
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                GROUP BY browser
            ),
            total AS (
                SELECT SUM(count) as total FROM browser_counts
            )
            SELECT
                bc.browser as "browser!",
                bc.count as "count!",
                CASE WHEN t.total > 0
                    THEN (bc.count::float / t.total * 100.0)
                    ELSE 0.0
                END as "percentage!"
            FROM browser_counts bc, total t
            ORDER BY bc.count DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_device_breakdown(&self, limit: i64) -> Result<Vec<DeviceBreakdown>> {
        sqlx::query_as!(
            DeviceBreakdown,
            r#"
            WITH device_counts AS (
                SELECT
                    COALESCE(device_type, 'Unknown') as device_type,
                    COUNT(*) as count
                FROM user_sessions
                WHERE started_at >= NOW() - INTERVAL '7 days'
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                GROUP BY device_type
            ),
            total AS (
                SELECT SUM(count) as total FROM device_counts
            )
            SELECT
                dc.device_type as "device_type!",
                dc.count as "count!",
                CASE WHEN t.total > 0
                    THEN (dc.count::float / t.total * 100.0)
                    ELSE 0.0
                END as "percentage!"
            FROM device_counts dc, total t
            ORDER BY dc.count DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_geographic_breakdown(&self, limit: i64) -> Result<Vec<GeographicBreakdown>> {
        sqlx::query_as!(
            GeographicBreakdown,
            r#"
            WITH country_counts AS (
                SELECT
                    COALESCE(country, 'Unknown') as country,
                    COUNT(*) as count
                FROM user_sessions
                WHERE started_at >= NOW() - INTERVAL '7 days'
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                GROUP BY country
            ),
            total AS (
                SELECT SUM(count) as total FROM country_counts
            )
            SELECT
                cc.country as "country!",
                cc.count as "count!",
                CASE WHEN t.total > 0
                    THEN (cc.count::float / t.total * 100.0)
                    ELSE 0.0
                END as "percentage!"
            FROM country_counts cc, total t
            ORDER BY cc.count DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_bot_traffic_stats(&self) -> Result<BotTrafficStats> {
        sqlx::query_as!(
            BotTrafficStats,
            r#"
            SELECT
                COUNT(*) as "total_requests!",
                COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true OR is_scanner = true) as "bot_requests!",
                COUNT(*) FILTER (WHERE is_bot = false AND is_scanner = false AND is_behavioral_bot = false) as "human_requests!",
                CASE WHEN COUNT(*) > 0
                    THEN (COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true OR is_scanner = true)::float / COUNT(*)::float * 100.0)
                    ELSE 0.0
                END as "bot_percentage!"
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '7 days'
            "#
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
