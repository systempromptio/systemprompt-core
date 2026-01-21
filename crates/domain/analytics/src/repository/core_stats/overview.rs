use anyhow::Result;
use chrono::{Duration, Utc};

use super::CoreStatsRepository;
use crate::models::{CostOverview, PlatformOverview, UserMetricsWithTrends};

impl CoreStatsRepository {
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
}
