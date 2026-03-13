use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::models::cli::{BotTotalsRow, BotTypeRow, DeviceRow, GeoRow, TrafficSourceRow};

#[derive(Debug)]
pub struct TrafficAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl TrafficAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_sources(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        engaged_only: bool,
    ) -> Result<Vec<TrafficSourceRow>> {
        if engaged_only {
            sqlx::query_as!(
                TrafficSourceRow,
                r#"
                SELECT
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                  AND landing_page IS NOT NULL AND request_count > 0
                GROUP BY referrer_source
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
                TrafficSourceRow,
                r#"
                SELECT
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                GROUP BY referrer_source
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
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                  AND landing_page IS NOT NULL AND request_count > 0
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
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
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
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
                  AND landing_page IS NOT NULL AND request_count > 0
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
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                  AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
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
        engaged_only: bool,
    ) -> Result<BotTotalsRow> {
        if engaged_only {
            sqlx::query_as!(
                BotTotalsRow,
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE is_bot = false AND is_behavioral_bot = false AND is_scanner = false AND landing_page IS NOT NULL AND request_count > 0)::bigint as "human!",
                    COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true OR is_scanner = true OR landing_page IS NULL OR request_count = 0)::bigint as "bot!"
                FROM user_sessions
                WHERE started_at >= $1 AND started_at < $2
                "#,
                start,
                end
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                BotTotalsRow,
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE is_bot = false AND is_behavioral_bot = false AND is_scanner = false)::bigint as "human!",
                    COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true OR is_scanner = true)::bigint as "bot!"
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
                CASE
                    WHEN user_agent ILIKE '%googlebot%' OR user_agent ILIKE '%google-inspectiontool%' OR user_agent ILIKE '%adsbot-google%' THEN 'Google'
                    WHEN user_agent ILIKE '%bingbot%' OR user_agent ILIKE '%bingpreview%' OR user_agent ILIKE '%msnbot%' THEN 'Bing'
                    WHEN user_agent ILIKE '%chatgpt%' OR user_agent ILIKE '%gptbot%' THEN 'OpenAI'
                    WHEN user_agent ILIKE '%claude%' OR user_agent ILIKE '%anthropic%' THEN 'Anthropic'
                    WHEN user_agent ILIKE '%perplexity%' THEN 'Perplexity'
                    WHEN user_agent ILIKE '%baiduspider%' THEN 'Baidu'
                    WHEN user_agent ILIKE '%yandexbot%' THEN 'Yandex'
                    WHEN user_agent ILIKE '%facebookexternalhit%' OR user_agent ILIKE '%facebot%' OR user_agent ILIKE '%meta-externalagent%' THEN 'Meta'
                    WHEN user_agent ILIKE '%twitterbot%' THEN 'Twitter/X'
                    WHEN user_agent ILIKE '%linkedinbot%' THEN 'LinkedIn'
                    WHEN user_agent ILIKE '%semrushbot%' OR user_agent ILIKE '%ahrefsbot%' OR user_agent ILIKE '%mj12bot%' OR user_agent ILIKE '%dotbot%' THEN 'SEO Crawlers'
                    WHEN user_agent ILIKE '%bytespider%' THEN 'ByteDance'
                    WHEN user_agent ILIKE '%amazonbot%' OR user_agent ILIKE '%applebot%' THEN 'Tech Giants'
                    WHEN user_agent ILIKE '%python%' OR user_agent ILIKE '%scrapy%' OR user_agent ILIKE '%httpx%' THEN 'Python Scrapers'
                    WHEN user_agent ILIKE '%curl%' OR user_agent ILIKE '%wget%' OR user_agent ILIKE '%node-fetch%' OR user_agent ILIKE '%axios%' THEN 'CLI/HTTP Tools'
                    WHEN user_agent ILIKE '%headless%' OR user_agent ILIKE '%phantom%' OR user_agent ILIKE '%selenium%' OR user_agent ILIKE '%puppeteer%' THEN 'Headless Browsers'
                    WHEN user_agent ILIKE '%uptimerobot%' OR user_agent ILIKE '%pingdom%' OR user_agent ILIKE '%statuscake%' OR user_agent ILIKE '%lighthouse%' THEN 'Monitoring'
                    WHEN is_behavioral_bot = true THEN 'Behavioral Bot'
                    WHEN is_scanner = true THEN 'Scanner'
                    ELSE 'Other'
                END as "bot_type",
                COUNT(*)::bigint as "count!"
            FROM user_sessions
            WHERE started_at >= $1 AND started_at < $2
              AND (is_bot = true OR is_behavioral_bot = true OR is_scanner = true)
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
