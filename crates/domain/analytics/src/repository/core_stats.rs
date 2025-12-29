use anyhow::Result;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;

use crate::models::{
    ActivityTrend, BotTrafficStats, BrowserBreakdown, ContentStat, CostOverview, DeviceBreakdown,
    GeographicBreakdown, PlatformOverview, RecentConversation, TopAgent, TopTool, TopUser,
    UserMetricsWithTrends,
};

#[derive(Debug)]
pub struct CoreStatsRepository {
    pool: Arc<PgPool>,
}

impl CoreStatsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_platform_overview(&self) -> Result<PlatformOverview> {
        let now = Utc::now();
        let cutoff_24h = now - Duration::hours(24);
        let cutoff_7d = now - Duration::days(7);
        sqlx::query_as!(
            PlatformOverview,
            r#"
            SELECT
                (SELECT COUNT(*) FROM users WHERE status != 'deleted') as "total_users!",
                (SELECT COUNT(DISTINCT user_id) FROM user_sessions WHERE last_activity_at > $1) as "active_users_24h!",
                (SELECT COUNT(DISTINCT user_id) FROM user_sessions WHERE last_activity_at > $2) as "active_users_7d!",
                (SELECT COUNT(*) FROM user_sessions) as "total_sessions!",
                (SELECT COUNT(*) FROM user_sessions WHERE ended_at IS NULL) as "active_sessions!",
                (SELECT COUNT(*) FROM user_contexts) as "total_contexts!",
                (SELECT COUNT(*) FROM agent_tasks) as "total_tasks!",
                (SELECT COUNT(*) FROM ai_requests) as "total_ai_requests!"
            "#,
            cutoff_24h,
            cutoff_7d
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_cost_overview(&self) -> Result<CostOverview> {
        let now = Utc::now();
        let since_hours_24 = now - Duration::hours(24);
        let since_days_7 = now - Duration::days(7);
        let since_days_30 = now - Duration::days(30);
        sqlx::query_as!(
            CostOverview,
            r#"
            SELECT
                COALESCE(SUM(cost_cents)::float / 100.0, 0.0) as "total_cost!",
                COALESCE(SUM(cost_cents) FILTER (WHERE created_at > $1)::float / 100.0, 0.0) as "cost_24h!",
                COALESCE(SUM(cost_cents) FILTER (WHERE created_at > $2)::float / 100.0, 0.0) as "cost_7d!",
                COALESCE(SUM(cost_cents) FILTER (WHERE created_at > $3)::float / 100.0, 0.0) as "cost_30d!",
                COALESCE(AVG(cost_cents)::float / 100.0, 0.0) as "avg_cost_per_request!"
            FROM ai_requests
            "#,
            since_hours_24,
            since_days_7,
            since_days_30
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_activity_trend(&self, days: i32) -> Result<Vec<ActivityTrend>> {
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        sqlx::query_as!(
            ActivityTrend,
            r#"
            SELECT
                date_trunc('day', gs.date) as "date!",
                COALESCE(s.sessions, 0) as "sessions!",
                COALESCE(c.contexts, 0) as "contexts!",
                COALESCE(t.tasks, 0) as "tasks!",
                COALESCE(a.ai_requests, 0) as "ai_requests!",
                COALESCE(e.tool_executions, 0) as "tool_executions!"
            FROM generate_series($1::timestamptz, NOW(), '1 day') gs(date)
            LEFT JOIN (
                SELECT date_trunc('day', started_at) as day, COUNT(*) as sessions
                FROM user_sessions WHERE started_at > $1
                GROUP BY 1
            ) s ON s.day = date_trunc('day', gs.date)
            LEFT JOIN (
                SELECT date_trunc('day', created_at) as day, COUNT(*) as contexts
                FROM user_contexts WHERE created_at > $1
                GROUP BY 1
            ) c ON c.day = date_trunc('day', gs.date)
            LEFT JOIN (
                SELECT date_trunc('day', created_at) as day, COUNT(*) as tasks
                FROM agent_tasks WHERE created_at > $1
                GROUP BY 1
            ) t ON t.day = date_trunc('day', gs.date)
            LEFT JOIN (
                SELECT date_trunc('day', created_at) as day, COUNT(*) as ai_requests
                FROM ai_requests WHERE created_at > $1
                GROUP BY 1
            ) a ON a.day = date_trunc('day', gs.date)
            LEFT JOIN (
                SELECT date_trunc('day', created_at) as day, COUNT(*) as tool_executions
                FROM mcp_tool_executions WHERE created_at > $1
                GROUP BY 1
            ) e ON e.day = date_trunc('day', gs.date)
            ORDER BY date ASC
            "#,
            cutoff
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_users(&self, limit: i64) -> Result<Vec<TopUser>> {
        sqlx::query_as!(
            TopUser,
            r#"
            SELECT
                u.id as user_id,
                u.name as user_name,
                COUNT(DISTINCT s.session_id) as "session_count!",
                COUNT(DISTINCT t.task_id) as "task_count!",
                COUNT(DISTINCT a.request_id) as "ai_request_count!",
                COALESCE(SUM(a.cost_cents)::float / 100.0, 0.0) as "total_cost!"
            FROM users u
            LEFT JOIN user_sessions s ON s.user_id = u.id
            LEFT JOIN agent_tasks t ON t.user_id = u.id
            LEFT JOIN ai_requests a ON a.user_id = u.id
            WHERE u.status NOT IN ('deleted', 'temporary') AND NOT ('anonymous' = ANY(u.roles))
            GROUP BY u.id, u.name
            ORDER BY "ai_request_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_agents(&self, limit: i64) -> Result<Vec<TopAgent>> {
        sqlx::query_as!(
            TopAgent,
            r#"
            SELECT
                agent_name as "agent_name!",
                COUNT(*) as "task_count!",
                COALESCE(
                    COUNT(*) FILTER (WHERE status = 'completed')::float / NULLIF(COUNT(*), 0),
                    0.0
                ) as "success_rate!",
                COALESCE(AVG(EXTRACT(EPOCH FROM (updated_at - created_at)) * 1000)::bigint, 0) as "avg_duration_ms!"
            FROM agent_tasks
            WHERE agent_name IS NOT NULL
            GROUP BY agent_name
            ORDER BY "task_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_tools(&self, limit: i64) -> Result<Vec<TopTool>> {
        sqlx::query_as!(
            TopTool,
            r#"
            SELECT
                tool_name,
                COUNT(*) as "execution_count!",
                COALESCE(
                    COUNT(*) FILTER (WHERE status = 'success')::float / NULLIF(COUNT(*), 0),
                    0.0
                ) as "success_rate!",
                COALESCE(AVG(execution_time_ms), 0)::bigint as "avg_duration_ms!"
            FROM mcp_tool_executions
            GROUP BY tool_name
            ORDER BY "execution_count!" DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_user_metrics_with_trends(&self) -> Result<UserMetricsWithTrends> {
        let now = Utc::now();
        let since_hours_24 = now - Duration::hours(24);
        let since_hours_48 = now - Duration::hours(48);
        let since_days_7 = now - Duration::days(7);
        let since_days_14 = now - Duration::days(14);
        let since_days_30 = now - Duration::days(30);
        let since_days_60 = now - Duration::days(60);

        sqlx::query_as!(
            UserMetricsWithTrends,
            r#"
            SELECT
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $1) as "count_24h!",
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $2) as "count_7d!",
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $3) as "count_30d!",
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $4 AND last_activity_at <= $1) as "prev_24h!",
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $5 AND last_activity_at <= $2) as "prev_7d!",
                COUNT(DISTINCT user_id) FILTER (WHERE last_activity_at > $6 AND last_activity_at <= $3) as "prev_30d!"
            FROM user_sessions
            "#,
            since_hours_24,
            since_days_7,
            since_days_30,
            since_hours_48,
            since_days_14,
            since_days_60
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_recent_conversations(&self, limit: i64) -> Result<Vec<RecentConversation>> {
        sqlx::query_as!(
            RecentConversation,
            r#"
            SELECT
                uc.context_id as "context_id!",
                COALESCE(at.agent_name, 'unknown') as "agent_name!",
                COALESCE(u.name, 'anonymous') as "user_name!",
                COALESCE(at.status, 'unknown') as "status!",
                COALESCE((
                    SELECT COUNT(*)
                    FROM task_messages tm
                    JOIN agent_tasks at2 ON tm.task_id = at2.task_id
                    WHERE at2.context_id = uc.context_id
                ), 0) as "message_count!",
                uc.created_at as "started_at!"
            FROM user_contexts uc
            LEFT JOIN agent_tasks at ON at.context_id = uc.context_id
            LEFT JOIN users u ON u.id = uc.user_id
            ORDER BY uc.created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_content_stats(&self, limit: i64) -> Result<Vec<ContentStat>> {
        sqlx::query_as!(
            ContentStat,
            r#"
            SELECT
                mc.title as "title!",
                mc.slug as "slug!",
                COUNT(ae.id) FILTER (WHERE ae.timestamp >= NOW() - INTERVAL '5 minutes') as "views_5m!",
                COUNT(ae.id) FILTER (WHERE ae.timestamp >= NOW() - INTERVAL '1 hour') as "views_1h!",
                COUNT(ae.id) FILTER (WHERE ae.timestamp >= NOW() - INTERVAL '1 day') as "views_1d!",
                COUNT(ae.id) FILTER (WHERE ae.timestamp >= NOW() - INTERVAL '7 days') as "views_7d!",
                COUNT(ae.id) FILTER (WHERE ae.timestamp >= NOW() - INTERVAL '30 days') as "views_30d!"
            FROM markdown_content mc
            LEFT JOIN analytics_events ae ON ae.endpoint = 'GET /' || mc.source_id || '/' || mc.slug
                AND ae.event_type = 'page_view'
            GROUP BY mc.id, mc.title, mc.slug
            ORDER BY "views_7d!" DESC NULLS LAST
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

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
                COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true) as "bot_requests!",
                COUNT(*) FILTER (WHERE is_bot = false AND is_scanner = false AND is_behavioral_bot = false) as "human_requests!",
                CASE WHEN COUNT(*) > 0
                    THEN (COUNT(*) FILTER (WHERE is_bot = true OR is_behavioral_bot = true)::float / COUNT(*)::float * 100.0)
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
