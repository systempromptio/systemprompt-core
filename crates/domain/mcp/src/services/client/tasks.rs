//! Client-side task polling for the `io.modelcontextprotocol/tasks`
//! extension (SEP-2663).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use rmcp::model::{CallToolResult, CreateTaskResult, GetTaskParams, TaskPayload};
use rmcp::service::{RoleClient, RunningService};

use crate::error::{McpDomainError, McpDomainResult};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);

// Why: a server that omits both ttlMs and pollIntervalMs could otherwise pin
// this client in an unbounded poll loop; the deadline is ours, not the spec's.
const MAX_POLL_BUDGET: Duration = Duration::from_secs(600);

pub(super) async fn poll_task_to_completion<S>(
    client: &RunningService<RoleClient, S>,
    server: &str,
    created: CreateTaskResult,
) -> McpDomainResult<CallToolResult>
where
    S: rmcp::Service<RoleClient>,
{
    let task_id = created.task.task_id.clone();
    let poll_interval = created
        .task
        .poll_interval_ms
        .map_or(DEFAULT_POLL_INTERVAL, Duration::from_millis);
    let budget = created.task.ttl_ms.map_or(MAX_POLL_BUDGET, |ttl| {
        Duration::from_millis(ttl).min(MAX_POLL_BUDGET)
    });
    let deadline = tokio::time::Instant::now() + budget;

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(McpDomainError::Timeout {
                server: server.to_owned(),
                after_ms: u64::try_from(budget.as_millis()).unwrap_or(u64::MAX),
            });
        }
        tokio::time::sleep(poll_interval).await;

        let polled = client
            .get_task(GetTaskParams::new(task_id.clone()))
            .await
            .map_err(|e| McpDomainError::ToolExecutionFailed(format!("tasks/get failed: {e}")))?;

        match polled.task.payload {
            TaskPayload::Working => {},
            TaskPayload::InputRequired { .. } => {
                tracing::debug!(task_id = %task_id, "Task awaiting input; continuing to poll");
            },
            TaskPayload::Completed { result } => {
                return serde_json::from_value(serde_json::Value::Object(result)).map_err(|e| {
                    McpDomainError::ToolExecutionFailed(format!(
                        "task completed with a non-CallToolResult payload: {e}"
                    ))
                });
            },
            TaskPayload::Failed { error } => {
                return Err(McpDomainError::ToolExecutionFailed(format!(
                    "task failed: {}",
                    serde_json::Value::Object(error)
                )));
            },
            TaskPayload::Cancelled => {
                return Err(McpDomainError::ToolExecutionFailed(
                    "task was cancelled by the server".to_owned(),
                ));
            },
            other => {
                tracing::warn!(status = ?other.status(), "Unrecognized task payload; continuing to poll");
            },
        }
    }
}
