//! A2A messages handed to a headless agent run.
//!
//! An agent whose `metadata.agent_run` holds a `RunRequest` template answers
//! no message inline when the host wired an `AgentRunLauncher`: the message
//! text becomes the template's `prompt`, a `Submitted` task is persisted and
//! returned, and the run moves the task through `Working` to its terminal
//! state in the same transactions that move the run. `tasks/cancel` on such a
//! task cancels the run; the run, not this handler, records `Canceled`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentName, TaskId};
use systemprompt_models::RequestContext;
use systemprompt_traits::AgentRunLaunch;

use super::non_streaming::RequestFailure;
use crate::models::a2a::{Message, Task};
use crate::repository::task::RepoCreateTaskParams;
use crate::services::a2a_server::handlers::state::AgentHandlerState;
use crate::services::a2a_server::processing::message::extract_message_content;
use crate::services::a2a_server::processing::message::message_handler::{
    new_submitted_task, resolve_task_id,
};

pub const AGENT_RUN_ORIGIN_KIND: &str = "a2a";

pub(super) async fn launch_if_configured(
    message: &Message,
    agent_name: &AgentName,
    state: &AgentHandlerState,
    context: &RequestContext,
) -> Result<Option<Task>, RequestFailure> {
    let Some(launcher) = state.agent_state.agent_run_launcher() else {
        return Ok(None);
    };
    let Some(mut request) = state.config.read().await.metadata.agent_run.clone() else {
        return Ok(None);
    };
    let Some(fields) = request.as_object_mut() else {
        return Err(RequestFailure::InvalidParams(format!(
            "agent {agent_name} metadata.agent_run is not an object"
        )));
    };
    let (prompt, _) = extract_message_content(message);
    fields.insert("prompt".to_owned(), serde_json::Value::String(prompt));

    let task_repo = &state.agent_state.repositories().tasks;
    let task_id = resolve_task_id(message);
    let existing = task_repo
        .find_task(&task_id)
        .await
        .map_err(|e| RequestFailure::internal("Task lookup failed", e))?;
    if existing.is_some() {
        return Err(RequestFailure::InvalidParams(format!(
            "task {task_id} exists; an agent-run task cannot be continued"
        )));
    }

    let task = new_submitted_task(&task_id, &message.context_id, agent_name);
    task_repo
        .create_task(RepoCreateTaskParams {
            task: &task,
            user_id: context.user_id(),
            session_id: context.session_id(),
            trace_id: context.trace_id(),
            agent_name,
        })
        .await
        .map_err(|e| RequestFailure::internal("Failed to persist task", e))?;

    let launch = AgentRunLaunch {
        user_id: context.user_id().clone(),
        task_id: task_id.clone(),
        context_id: message.context_id.clone(),
        request,
        origin_kind: AGENT_RUN_ORIGIN_KIND.to_owned(),
        origin_id: task_id.to_string(),
    };
    match launcher.enqueue(launch).await {
        Ok(run_id) => {
            tracing::info!(task_id = %task_id, run_id = %run_id, "Queued agent run for task");
            Ok(Some(task))
        },
        Err(e) => {
            let failed = task_repo
                .update_task_failed_with_error(&task_id, &e.to_string(), &chrono::Utc::now())
                .await;
            if let Err(mark) = failed {
                tracing::error!(task_id = %task_id, error = %mark, "Failed to mark task failed");
            }
            Err(RequestFailure::internal("Failed to queue agent run", e))
        },
    }
}

pub(super) async fn cancel_run(
    task_id: &TaskId,
    state: &AgentHandlerState,
) -> Result<bool, RequestFailure> {
    let Some(launcher) = state.agent_state.agent_run_launcher() else {
        return Ok(false);
    };
    let run = launcher
        .run_for_task(task_id)
        .await
        .map_err(|e| RequestFailure::internal("Agent run lookup failed", e))?;
    let Some(run_id) = run else {
        return Ok(false);
    };
    launcher
        .cancel(&run_id)
        .await
        .map_err(|e| RequestFailure::internal("Failed to cancel agent run", e))?;
    tracing::info!(task_id = %task_id, run_id = %run_id, "Cancellation requested for agent run");
    Ok(true)
}
