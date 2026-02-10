use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, PlannedTool, StepContent, StepId, StepStatus};

fn parse_step(
    step_id: String,
    task_id: String,
    status: String,
    content: serde_json::Value,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    duration_ms: Option<i32>,
    error_message: Option<String>,
) -> Result<ExecutionStep> {
    let status = status
        .parse::<StepStatus>()
        .map_err(|e| anyhow::anyhow!("Invalid status: {}", e))?;
    let content: StepContent =
        serde_json::from_value(content).map_err(|e| anyhow::anyhow!("Invalid content: {}", e))?;
    Ok(ExecutionStep {
        step_id: step_id.into(),
        task_id: task_id.into(),
        status,
        started_at,
        completed_at,
        duration_ms,
        error_message,
        content,
    })
}

#[derive(Debug, Clone)]
pub struct ExecutionStepRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ExecutionStepRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create(&self, step: &ExecutionStep) -> Result<()> {
        let step_id_str = step.step_id.as_str();
        let task_id = &step.task_id;
        let status_str = step.status.to_string();
        let step_type_str = step.content.step_type().to_string();
        let title = step.content.title();
        let content_json =
            serde_json::to_value(&step.content).context("Failed to serialize step content")?;
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
        .context("Failed to create execution step")?;
        Ok(())
    }

    pub async fn get(&self, step_id: &StepId) -> Result<Option<ExecutionStep>> {
        let step_id_str = step_id.as_str();
        let row = sqlx::query!(
            r#"SELECT step_id, task_id, status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message
                FROM task_execution_steps WHERE step_id = $1"#,
            step_id_str
        )
        .fetch_optional(&*self.pool)
        .await
        .context(format!("Failed to get execution step: {step_id}"))?;
        row.map(|r| {
            parse_step(
                r.step_id,
                r.task_id,
                r.status,
                r.content,
                r.started_at,
                r.completed_at,
                r.duration_ms,
                r.error_message,
            )
        })
        .transpose()
    }

    pub async fn list_by_task(&self, task_id: &TaskId) -> Result<Vec<ExecutionStep>> {
        let rows = sqlx::query!(
            r#"SELECT step_id, task_id, status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message
                FROM task_execution_steps WHERE task_id = $1 ORDER BY started_at ASC"#,
            task_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .context(format!(
            "Failed to list execution steps for task: {}",
            task_id
        ))?;
        rows.into_iter()
            .map(|r| {
                parse_step(
                    r.step_id,
                    r.task_id,
                    r.status,
                    r.content,
                    r.started_at,
                    r.completed_at,
                    r.duration_ms,
                    r.error_message,
                )
            })
            .collect()
    }

    pub async fn complete_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        tool_result: Option<serde_json::Value>,
    ) -> Result<()> {
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
            .context(format!("Failed to complete execution step: {step_id}"))?;
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
            .context(format!("Failed to complete execution step: {step_id}"))?;
        }

        Ok(())
    }

    pub async fn fail_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        error_message: &str,
    ) -> Result<()> {
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
        .context(format!("Failed to fail execution step: {step_id}"))?;

        Ok(())
    }

    pub async fn fail_in_progress_steps_for_task(
        &self,
        task_id: &TaskId,
        error_message: &str,
    ) -> Result<u64> {
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
        .context(format!(
            "Failed to fail in-progress steps for task: {}",
            task_id
        ))?;

        Ok(result.rows_affected())
    }

    pub async fn complete_planning_step(
        &self,
        step_id: &StepId,
        started_at: DateTime<Utc>,
        reasoning: Option<String>,
        planned_tools: Option<Vec<PlannedTool>>,
    ) -> Result<ExecutionStep> {
        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds() as i32;
        let step_id_str = step_id.as_str();
        let status_str = StepStatus::Completed.to_string();

        let content = StepContent::planning(reasoning, planned_tools);
        let content_json =
            serde_json::to_value(&content).context("Failed to serialize planning content")?;

        let row = sqlx::query!(
            r#"UPDATE task_execution_steps SET
                status = $2,
                completed_at = $3,
                duration_ms = $4,
                content = $5
            WHERE step_id = $1
            RETURNING step_id, task_id, status, content,
                    started_at as "started_at!", completed_at, duration_ms, error_message"#,
            step_id_str,
            status_str,
            completed_at,
            duration_ms,
            content_json
        )
        .fetch_one(&*self.write_pool)
        .await
        .context(format!("Failed to complete planning step: {step_id}"))?;

        parse_step(
            row.step_id,
            row.task_id,
            row.status,
            row.content,
            row.started_at,
            row.completed_at,
            row.duration_ms,
            row.error_message,
        )
    }

    pub async fn mcp_execution_id_exists(&self, mcp_execution_id: &str) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1) as "exists!""#,
            mcp_execution_id
        )
        .fetch_one(&*self.pool)
        .await
        .context("Failed to check mcp_execution_id existence")?;

        Ok(exists)
    }
}
