use anyhow::{anyhow, Result};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_database::{DatabaseProvider, DbPool, JsonRow, ToDbValue};

#[derive(Debug, Clone)]
pub struct AnalyticsQueryRepository {
    db_pool: DbPool,
}

impl AnalyticsQueryRepository {
    #[allow(clippy::unnecessary_wraps)]
    pub fn new(db_pool: &DbPool) -> Result<Self> {
        Ok(Self { db_pool: Arc::clone(db_pool) })
    }

    pub async fn get_ai_provider_usage(
        &self,
        days: i32,
        user_id: Option<&str>,
    ) -> Result<Vec<ProviderUsage>> {
        let base_query = r"
            SELECT
                provider,
                model,
                COUNT(*) as request_count,
                SUM(tokens_used) as total_tokens,
                SUM(cost_microdollars) as total_cost_microdollars,
                AVG(latency_ms) as avg_latency_ms,
                COUNT(DISTINCT user_id) as unique_users,
                COUNT(DISTINCT session_id) as unique_sessions
            FROM ai_requests
            WHERE created_at >= NOW() - INTERVAL '1 day' * $1
            ";

        let mut query = base_query.to_string();
        let mut params: Vec<Box<dyn ToDbValue>> = vec![Box::new(days)];
        let mut param_index = 2;

        let placeholder = |idx: &mut i32| {
            let placeholder = format!("${idx}");
            *idx += 1;
            placeholder
        };

        if let Some(uid) = user_id {
            query.push_str(&format!(" AND user_id = {}", placeholder(&mut param_index)));
            params.push(Box::new(uid.to_string()));
        }

        query.push_str(" GROUP BY provider, model ORDER BY request_count DESC");

        let param_refs: Vec<&dyn ToDbValue> = params.iter().map(|p| &**p).collect();

        let rows = self.db_pool.as_ref().fetch_all(&query, &param_refs).await?;

        rows.iter()
            .map(ProviderUsage::from_json_row)
            .collect::<Result<Vec<_>>>()
    }
}

#[derive(Debug, Serialize)]
pub struct ProviderUsage {
    pub provider: String,
    pub model: String,
    pub request_count: i32,
    pub total_tokens: Option<i32>,
    pub total_cost_microdollars: Option<i32>,
    pub avg_latency_ms: Option<f64>,
    pub unique_users: i32,
    pub unique_sessions: i32,
}

impl ProviderUsage {
    pub fn from_json_row(row: &JsonRow) -> Result<Self> {
        let provider = row
            .get("provider")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing provider"))?
            .to_string();

        let model = row
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing model"))?
            .to_string();

        let request_count = row
            .get("request_count")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| anyhow!("Missing request_count"))? as i32;

        let total_tokens = row
            .get("total_tokens")
            .and_then(serde_json::Value::as_i64)
            .map(|i| i as i32);

        let total_cost_microdollars = row
            .get("total_cost_microdollars")
            .and_then(serde_json::Value::as_i64)
            .map(|i| i as i32);

        let avg_latency_ms = row
            .get("avg_latency_ms")
            .and_then(serde_json::Value::as_f64);

        let unique_users = row
            .get("unique_users")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| anyhow!("Missing unique_users"))? as i32;

        let unique_sessions =
            row.get("unique_sessions")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| anyhow!("Missing unique_sessions"))? as i32;

        Ok(Self {
            provider,
            model,
            request_count,
            total_tokens,
            total_cost_microdollars,
            avg_latency_ms,
            unique_users,
            unique_sessions,
        })
    }
}
