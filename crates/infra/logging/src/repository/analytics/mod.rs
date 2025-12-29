use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{AgentId, ContextId, SessionId, TaskId, UserId};
use systemprompt_traits::{Repository as RepositoryTrait, RepositoryError};

#[derive(Debug, Clone)]
pub struct AnalyticsRepository {
    db_pool: DbPool,
}

impl RepositoryTrait for AnalyticsRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}

impl AnalyticsRepository {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn log_event(&self, event: &AnalyticsEvent) -> anyhow::Result<i64> {
        let pool = self
            .db_pool
            .pool_arc()
            .context("Failed to get database pool")?;

        let result = execute_insert(&pool, event).await?;
        Ok(i64::try_from(result).unwrap_or(i64::MAX))
    }
}

async fn execute_insert(
    pool: &std::sync::Arc<sqlx::PgPool>,
    event: &AnalyticsEvent,
) -> anyhow::Result<u64> {
    let params = EventParams::from(event);
    run_insert_query(pool, params).await
}

#[allow(clippy::cognitive_complexity)]
async fn run_insert_query(
    pool: &std::sync::Arc<sqlx::PgPool>,
    p: EventParams<'_>,
) -> anyhow::Result<u64> {
    sqlx::query!(
        r"
        INSERT INTO analytics_events
        (user_id, session_id, context_id, event_type, event_category, severity,
         endpoint, error_code, response_time_ms, agent_id, task_id, message, metadata, timestamp)
        VALUES
        ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ",
        p.user_id,
        p.session_id,
        p.context_id,
        p.event_type,
        p.event_category,
        p.severity,
        p.endpoint,
        p.error_code,
        p.response_time_ms,
        p.agent_id,
        p.task_id,
        p.message,
        p.metadata,
        p.timestamp
    )
    .execute(pool.as_ref())
    .await
    .map(|r| r.rows_affected())
    .context("Failed to log analytics event")
}

struct EventParams<'a> {
    user_id: &'a str,
    session_id: &'a str,
    context_id: &'a str,
    event_type: &'a str,
    event_category: &'a str,
    severity: &'a str,
    agent_id: Option<&'a str>,
    task_id: Option<&'a str>,
    endpoint: Option<&'a str>,
    message: Option<&'a str>,
    error_code: Option<i32>,
    response_time_ms: Option<i32>,
    metadata: String,
    timestamp: chrono::DateTime<Utc>,
}

impl<'a> From<&'a AnalyticsEvent> for EventParams<'a> {
    fn from(event: &'a AnalyticsEvent) -> Self {
        Self {
            user_id: event.user_id.as_str(),
            session_id: event.session_id.as_str(),
            context_id: event.context_id.as_str(),
            event_type: &event.event_type,
            event_category: &event.event_category,
            severity: &event.severity,
            agent_id: event.agent_id.as_ref().map(AgentId::as_str),
            task_id: event.task_id.as_ref().map(TaskId::as_str),
            endpoint: event.endpoint.as_deref(),
            message: event.message.as_deref(),
            error_code: event.error_code,
            response_time_ms: event.response_time_ms,
            metadata: event.metadata.to_string(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyticsEvent {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub event_type: String,
    pub event_category: String,
    pub severity: String,
    pub endpoint: Option<String>,
    pub error_code: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub agent_id: Option<AgentId>,
    pub task_id: Option<TaskId>,
    pub message: Option<String>,
    pub metadata: Value,
}
