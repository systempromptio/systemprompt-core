//! Task creation and context-agent tracking for `agent_tasks`.
//!
//! State transitions (with optimistic-concurrency guards) live in the
//! sibling `state` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::AgentName;
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Task;

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct CreateTaskParams<'a> {
    pub pool: &'a Arc<PgPool>,
    pub task: &'a Task,
    pub user_id: &'a systemprompt_identifiers::UserId,
    pub session_id: &'a systemprompt_identifiers::SessionId,
    pub trace_id: &'a systemprompt_identifiers::TraceId,
    pub agent_name: &'a AgentName,
}

pub async fn create_task(params: CreateTaskParams<'_>) -> Result<String, RepositoryError> {
    let CreateTaskParams {
        pool,
        task,
        user_id,
        session_id,
        trace_id,
        agent_name,
    } = params;
    let metadata_json = match task.metadata.as_ref() {
        Some(m) => serde_json::to_value(m)?,
        None => serde_json::json!({}),
    };

    let status = task.status.state.as_str();
    let task_id_str = task.id.as_str();
    let context_id_str = task.context_id.as_str();
    let user_id_str = user_id.as_ref();
    let session_id_str = session_id.as_ref();
    let trace_id_str = trace_id.as_ref();

    sqlx::query!(
        r#"INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, session_id, trace_id, metadata, agent_name)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        task_id_str,
        context_id_str,
        status,
        task.status.timestamp,
        user_id_str,
        session_id_str,
        trace_id_str,
        metadata_json,
        agent_name.as_str()
    )
    .execute(pool.as_ref())
    .await?;

    Ok(task.id.to_string())
}

pub async fn track_agent_in_context(
    pool: &Arc<PgPool>,
    context_id: &systemprompt_identifiers::ContextId,
    agent_name: &AgentName,
) -> Result<(), RepositoryError> {
    let context_id_str = context_id.as_str();
    sqlx::query!(
        r#"INSERT INTO context_agents (context_id, agent_name) VALUES ($1, $2)
        ON CONFLICT (context_id, agent_name) DO NOTHING"#,
        context_id_str,
        agent_name.as_str()
    )
    .execute(pool.as_ref())
    .await?;

    Ok(())
}
