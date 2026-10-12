//! [`AgentRunLauncher`]: the seam through which the A2A server hands a task
//! to a headless coding-agent run without depending on the crate that runs it.
//!
//! The agent domain sits below the app layer that owns `agent_runs`, so it
//! holds an `Option<Arc<dyn AgentRunLauncher>>` that entry wires from the
//! agent-runs extension. [`AgentRunLaunch::request`] is that crate's
//! `RunRequest` as JSON, opaque here. A launched run drives its task
//! `Submitted → Working → Completed/Failed/Canceled` itself.
//! [`AgentRunLauncher::run_for_task`] lets `tasks/cancel` find the run behind
//! a task, since the agent domain cannot read `agent_runs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId, UserId};

/// One run to queue on behalf of an A2A task.
#[derive(Debug, Clone)]
pub struct AgentRunLaunch {
    pub user_id: UserId,
    pub task_id: TaskId,
    pub context_id: ContextId,
    // JSON: the agent-runs crate's `RunRequest`, which this crate cannot name.
    pub request: serde_json::Value,
    pub origin_kind: String,
    pub origin_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentRunLaunchError {
    #[error("invalid run request: {0}")]
    InvalidRequest(String),
    #[error("agent run {0} not found")]
    NotFound(AgentRunId),
    #[error("agent run launcher unavailable")]
    Unavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl AgentRunLaunchError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Unavailable(Box::new(source))
    }
}

/// Queues and cancels agent runs for A2A tasks.
// Why: launchers are held as `Arc<dyn AgentRunLauncher>`; native `async fn`
// in traits is not `dyn`-compatible.
#[async_trait]
pub trait AgentRunLauncher: Send + Sync {
    async fn enqueue(&self, request: AgentRunLaunch) -> Result<AgentRunId, AgentRunLaunchError>;

    async fn cancel(&self, run_id: &AgentRunId) -> Result<(), AgentRunLaunchError>;

    async fn run_for_task(&self, task_id: &TaskId)
    -> Result<Option<AgentRunId>, AgentRunLaunchError>;
}

pub type DynAgentRunLauncher = std::sync::Arc<dyn AgentRunLauncher>;
