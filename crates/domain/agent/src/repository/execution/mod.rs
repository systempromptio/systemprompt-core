//! Repository for `task_execution_steps` — per-task tool calls and intermediate
//! state.

mod parse;

use systemprompt_traits::RepositoryError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, PlannedTool, StepContent, StepId, StepStatus};

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

    pub async fn create(&self, step: &ExecutionStep) -> Result<(), RepositoryError> {
        let step_id_str = step.step_id.as_str();
        let task_id = &step.task_id;
        let status_str = step.status.to_string();
        let step_type_str = step.content.step_type().to_string();
        let title = step.content.title();
        let content_json =
            serde_json::to_value(&step.content).map_err(|e| RepositoryError::Internal(format!("Failed to serialize step content: {e}")))?;
        sqlx::query!(
            r#"INSERT INTO task_execution_steps (
                step_id, task_id, step_type, title, status, content, started_at, completed_at, duration_ms, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            step_id_str,
            task_id.as_str(),
            step_type_str,
            title,
            status_str,
            content_json,
            step.started_at,
            step.completed_at,
            step.duration_ms,
            step.error_message
        )
        .execute(&*self.write_pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("Failed to create execution step: {e}")))?;
        Ok(())
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
        .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!("Failed to get execution step: {step_id}"))))?;
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

    pub async fn list_by_task(&self, task_id: &TaskId) -> Result<Vec<ExecutionStep>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT step_id, task_id as "task_id!: TaskId", status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message
                FROM task_execution_steps WHERE task_id = $1 ORDER BY started_at ASC"#,
            task_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!(
            "Failed to list execution steps for task: {}",
            task_id
        ))))?;
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

    pub async fn complete_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        tool_result: Option<serde_json::Value>,
    ) -> Result<(), RepositoryError> {
        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds() as i32;
        let step_id_str = step_id.as_str();
        let status_str = StepStatus::Completed.to_string();

        if let Some(result) = tool_result {
            sqlx::query!(
                r#"UPDATE task_execution_steps SET
                    status = $2,
                    completed_at = $3,
                    duration_ms = $4,
                    content = jsonb_set(content, '{tool_result}', $5)
                WHERE step_id = $1"#,
                step_id_str,
                status_str,
                completed_at,
                duration_ms,
                result
            )
            .execute(&*self.write_pool)
            .await
            .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!("Failed to complete execution step: {step_id}"))))?;
        } else {
            sqlx::query!(
                r#"UPDATE task_execution_steps SET
                    status = $2,
                    completed_at = $3,
                    duration_ms = $4
                WHERE step_id = $1"#,
                step_id_str,
                status_str,
                completed_at,
                duration_ms
            )
            .execute(&*self.write_pool)
            .await
            .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!("Failed to complete execution step: {step_id}"))))?;
        }

        Ok(())
    }

    pub async fn fail_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        error_message: &str,
    ) -> Result<(), RepositoryError> {
        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds() as i32;
        let step_id_str = step_id.as_str();
        let status_str = StepStatus::Failed.to_string();

        sqlx::query!(
            r#"UPDATE task_execution_steps SET
                status = $2,
                completed_at = $3,
                duration_ms = $4,
                error_message = $5
            WHERE step_id = $1"#,
            step_id_str,
            status_str,
            completed_at,
            duration_ms,
            error_message
        )
        .execute(&*self.write_pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!("Failed to fail execution step: {step_id}"))))?;

        Ok(())
    }

    pub async fn fail_in_progress_steps_for_task(
        &self,
        task_id: &TaskId,
        error_message: &str,
    ) -> Result<u64, RepositoryError> {
        let completed_at = Utc::now();
        let in_progress_str = StepStatus::InProgress.to_string();
        let failed_str = StepStatus::Failed.to_string();
        let task_id_str = task_id.as_str();

        let result = sqlx::query!(
            r#"UPDATE task_execution_steps SET
                status = $3,
                completed_at = $4,
                error_message = $5
            WHERE task_id = $1 AND status = $2"#,
            task_id_str,
            in_progress_str,
            failed_str,
            completed_at,
            error_message
        )
        .execute(&*self.write_pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!(
            "Failed to fail in-progress steps for task: {}",
            task_id
        ))))?;

        Ok(result.rows_affected())
    }

    pub async fn complete_planning_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        reasoning: Option<String>,
        planned_tools: Option<Vec<PlannedTool>>,
    ) -> Result<ExecutionStep, RepositoryError> {
        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds() as i32;
        let step_id_str = step_id.as_str();
        let status_str = StepStatus::Completed.to_string();

        let content = StepContent::planning(reasoning, planned_tools);
        let content_json =
            serde_json::to_value(&content).map_err(|e| RepositoryError::Internal(format!("Failed to serialize planning content: {e}")))?;

        let row = sqlx::query!(
            r#"UPDATE task_execution_steps SET
                status = $2,
                completed_at = $3,
                duration_ms = $4,
                content = $5
            WHERE step_id = $1
            RETURNING step_id, task_id as "task_id!: TaskId", status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message"#,
            step_id_str,
            status_str,
            completed_at,
            duration_ms,
            content_json
        )
        .fetch_one(&*self.write_pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("{}: {e}", format!("Failed to complete planning step: {step_id}"))))?;

        parse_step(ParseStepParams {
            step_id: row.step_id,
            task_id: row.task_id,
            status: row.status,
            content: row.content,
            started_at: row.started_at,
            completed_at: row.completed_at,
            duration_ms: row.duration_ms,
            error_message: row.error_message,
        })
    }

    pub async fn mcp_execution_id_exists(&self, mcp_execution_id: &str) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1) as "exists!""#,
            mcp_execution_id
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| RepositoryError::Internal(format!("Failed to check mcp_execution_id existence: {e}")))?;

        Ok(exists)
    }
}
