//! Repository for `task_execution_steps` — per-task tool calls and intermediate
//! state.
//!
//! Read paths live here; write paths (create, complete, fail) live in the
//! `mutations` submodule.

mod mutations;
mod parse;

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{McpExecutionId, TaskId};
use systemprompt_models::{ExecutionStep, StepId};
use systemprompt_traits::RepositoryError;

use parse::{ParseStepParams, parse_step};

#[derive(Debug, Clone)]
pub struct ExecutionStepRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ExecutionStepRepository {
    pub fn new(db: &DbPool) -> Result<Self, crate::error::AgentError> {
        let pool = db
            .pool_arc()
            .map_err(|e| crate::error::AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| crate::error::AgentError::Init(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }

    pub async fn get(&self, step_id: &StepId) -> Result<Option<ExecutionStep>, RepositoryError> {
        let step_id_str = step_id.as_str();
        let row = sqlx::query!(
            r#"SELECT step_id, task_id as "task_id!: TaskId", status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message
                FROM task_execution_steps WHERE step_id = $1"#,
            step_id_str
        )
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            RepositoryError::Internal(format!("Failed to get execution step: {step_id}: {e}"))
        })?;
        row.map(|r| {
            parse_step(ParseStepParams {
                step_id: r.step_id,
                task_id: r.task_id,
                status: r.status,
                content: r.content,
                started_at: r.started_at,
                completed_at: r.completed_at,
                duration_ms: r.duration_ms,
                error_message: r.error_message,
            })
        })
        .transpose()
    }

    pub async fn list_by_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<ExecutionStep>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT step_id, task_id as "task_id!: TaskId", status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message
                FROM task_execution_steps WHERE task_id = $1 ORDER BY started_at ASC"#,
            task_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            RepositoryError::Internal(format!(
                "Failed to list execution steps for task: {task_id}: {e}"
            ))
        })?;
        rows.into_iter()
            .map(|r| {
                parse_step(ParseStepParams {
                    step_id: r.step_id,
                    task_id: r.task_id,
                    status: r.status,
                    content: r.content,
                    started_at: r.started_at,
                    completed_at: r.completed_at,
                    duration_ms: r.duration_ms,
                    error_message: r.error_message,
                })
            })
            .collect()
    }

    pub async fn mcp_execution_id_exists(
        &self,
        mcp_execution_id: &McpExecutionId,
    ) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1) as "exists!""#,
            mcp_execution_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("Failed to check mcp_execution_id existence: {e}")))?;

        Ok(exists)
    }
}
