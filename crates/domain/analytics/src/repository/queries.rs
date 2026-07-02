use crate::Result;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone)]
pub struct AnalyticsQueryRepository {
    pool: Arc<PgPool>,
}

impl AnalyticsQueryRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_ai_provider_usage(
        &self,
        days: i32,
        user_id: Option<&UserId>,
    ) -> Result<Vec<ProviderUsage>> {
        let user_filter = user_id.map(UserId::as_str);

        sqlx::query_as!(
            ProviderUsage,
            r#"
            SELECT
                provider AS "provider!",
                model AS "model!",
                COUNT(*)::int AS "request_count!",
                SUM(tokens_used)::int AS "total_tokens",
                SUM(cost_microdollars)::bigint AS "total_cost_microdollars",
                AVG(latency_ms)::float8 AS "avg_latency_ms",
                COUNT(DISTINCT user_id)::int AS "unique_users!",
                COUNT(DISTINCT session_id)::int AS "unique_sessions!"
            FROM ai_requests
            WHERE created_at >= NOW() - INTERVAL '1 day' * $1::int
              AND ($2::text IS NULL OR user_id = $2)
            GROUP BY provider, model
            ORDER BY COUNT(*) DESC
            "#,
            days,
            user_filter,
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}

#[derive(Debug, Serialize)]
pub struct ProviderUsage {
    pub provider: String,
    pub model: String,
    pub request_count: i32,
    pub total_tokens: Option<i32>,
    pub total_cost_microdollars: Option<i64>,
    pub avg_latency_ms: Option<f64>,
    pub unique_users: i32,
    pub unique_sessions: i32,
}
