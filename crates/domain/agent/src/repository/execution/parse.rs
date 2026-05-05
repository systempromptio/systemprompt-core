//! Row -> [`ExecutionStep`] parsing helper for the execution-step repository.

use anyhow::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, StepContent, StepStatus};

#[allow(missing_debug_implementations)]
pub(super) struct ParseStepParams {
    pub step_id: String,
    pub task_id: TaskId,
    pub status: String,
    pub content: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

pub(super) fn parse_step(params: ParseStepParams) -> Result<ExecutionStep> {
    let ParseStepParams {
        step_id,
        task_id,
        status,
        content,
        started_at,
        completed_at,
        duration_ms,
        error_message,
    } = params;
    let status = status
        .parse::<StepStatus>()
        .map_err(|e| anyhow::anyhow!("Invalid status: {}", e))?;
    let content: StepContent =
        serde_json::from_value(content).map_err(|e| anyhow::anyhow!("Invalid content: {}", e))?;
    Ok(ExecutionStep {
        step_id: step_id.into(),
        task_id,
        status,
        started_at,
        completed_at,
        duration_ms,
        error_message,
        content,
    })
}
