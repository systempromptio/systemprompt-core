use anyhow::Result;
use chrono::{Duration, Utc};

use super::CoreStatsRepository;
use crate::models::{ActivityTrend, ContentStat, RecentConversation};

impl CoreStatsRepository {
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
                FROM user_sessions WHERE started_at > $1 AND is_bot = false AND is_behavioral_bot = false AND is_scanner = false
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
}
