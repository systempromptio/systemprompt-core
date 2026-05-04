//! Execution step model — a single unit of an agent run with status,
//! timing, and per-kind content payload.

mod content;
mod enums;

pub use content::{PlannedTool, StepContent};
pub use enums::{StepId, StepStatus, StepType};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{SkillId, TaskId};

/// One unit of work produced by an agent run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStep {
    /// Stable step id.
    pub step_id: StepId,
    /// Task this step belongs to.
    pub task_id: TaskId,
    /// Lifecycle state.
    pub status: StepStatus,
    /// Wall-clock timestamp when the step started.
    pub started_at: DateTime<Utc>,
    /// Wall-clock timestamp when the step finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Elapsed duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    /// Error message attached to a failed step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Per-kind step content payload.
    pub content: StepContent,
}

impl ExecutionStep {
    /// Build a new step for `task_id` carrying `content`. Instant steps
    /// (everything except tool execution) are marked completed
    /// immediately with duration 0.
    #[must_use]
    pub fn new(task_id: TaskId, content: StepContent) -> Self {
        let status = if content.is_instant() {
            StepStatus::Completed
        } else {
            StepStatus::InProgress
        };
        let now = Utc::now();
        let (completed_at, duration_ms) = if content.is_instant() {
            (Some(now), Some(0))
        } else {
            (None, None)
        };

        Self {
            step_id: StepId::new(),
            task_id,
            status,
            started_at: now,
            completed_at,
            duration_ms,
            error_message: None,
            content,
        }
    }

    /// Build an `Understanding` step.
    #[must_use]
    pub fn understanding(task_id: TaskId) -> Self {
        Self::new(task_id, StepContent::understanding())
    }

    /// Build a `Planning` step.
    #[must_use]
    pub fn planning(
        task_id: TaskId,
        reasoning: Option<String>,
        planned_tools: Option<Vec<PlannedTool>>,
    ) -> Self {
        Self::new(task_id, StepContent::planning(reasoning, planned_tools))
    }

    /// Build a `SkillUsage` step.
    #[must_use]
    pub fn skill_usage(task_id: TaskId, skill_id: SkillId, skill_name: impl Into<String>) -> Self {
        Self::new(task_id, StepContent::skill_usage(skill_id, skill_name))
    }

    /// Build a `ToolExecution` step.
    #[must_use]
    pub fn tool_execution(
        task_id: TaskId,
        tool_name: impl Into<String>,
        tool_arguments: serde_json::Value,
    ) -> Self {
        Self::new(
            task_id,
            StepContent::tool_execution(tool_name, tool_arguments),
        )
    }

    /// Build a `Completion` step.
    #[must_use]
    pub fn completion(task_id: TaskId) -> Self {
        Self::new(task_id, StepContent::completion())
    }

    /// High-level kind of this step.
    #[must_use]
    pub const fn step_type(&self) -> StepType {
        self.content.step_type()
    }

    /// UI-friendly title.
    #[must_use]
    pub fn title(&self) -> String {
        self.content.title()
    }

    /// Borrow the underlying tool / skill name, if any.
    #[must_use]
    pub fn tool_name(&self) -> Option<&str> {
        self.content.tool_name()
    }

    /// Borrow the tool arguments, if any.
    #[must_use]
    pub const fn tool_arguments(&self) -> Option<&serde_json::Value> {
        self.content.tool_arguments()
    }

    /// Borrow the tool result, if any.
    #[must_use]
    pub const fn tool_result(&self) -> Option<&serde_json::Value> {
        self.content.tool_result()
    }

    /// Borrow planner reasoning, if any.
    #[must_use]
    pub fn reasoning(&self) -> Option<&str> {
        self.content.reasoning()
    }

    /// Mark this step completed and record the optional tool result.
    pub fn complete(&mut self, result: Option<serde_json::Value>) {
        let now = Utc::now();
        self.status = StepStatus::Completed;
        self.completed_at = Some(now);
        let duration = (now - self.started_at).num_milliseconds();
        self.duration_ms = Some(i32::try_from(duration).unwrap_or(i32::MAX));
        if let Some(r) = result {
            self.content = self.content.clone().with_tool_result(r);
        }
    }

    /// Mark this step failed with `error`.
    pub fn fail(&mut self, error: String) {
        let now = Utc::now();
        self.status = StepStatus::Failed;
        self.completed_at = Some(now);
        let duration = (now - self.started_at).num_milliseconds();
        self.duration_ms = Some(i32::try_from(duration).unwrap_or(i32::MAX));
        self.error_message = Some(error);
    }
}

/// In-flight step bookkeeping used while a step is executing.
#[derive(Debug, Clone)]
pub struct TrackedStep {
    /// Step identifier this tracker is following.
    pub step_id: StepId,
    /// When the step started.
    pub started_at: DateTime<Utc>,
}

/// Alias retained for compatibility with the original flat module shape.
pub type StepDetail = StepContent;
