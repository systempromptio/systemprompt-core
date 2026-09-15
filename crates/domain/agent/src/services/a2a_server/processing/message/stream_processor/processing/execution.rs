//! Strategy execution and artifact assembly for one pipeline run; a failure
//! marks in-progress steps failed before propagating.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::super::helpers::build_artifacts_from_results;
use crate::models::a2a::Artifact;
use crate::services::a2a_server::processing::strategies::{
    ExecutionContext, ExecutionResult, ExecutionStrategySelector,
};
use crate::services::shared::Result;
use systemprompt_models::AiMessage;

pub(super) async fn run_strategy(
    execution_context: ExecutionContext,
    ai_messages: Vec<AiMessage>,
) -> Result<ExecutionResult> {
    let has_tools = !execution_context
        .agent_runtime
        .mcp_servers
        .include
        .is_empty();
    tracing::info!(
        mcp_server_count = execution_context.agent_runtime.mcp_servers.include.len(),
        has_tools = has_tools,
        "Agent MCP server status"
    );

    let strategy = ExecutionStrategySelector::select_strategy(has_tools);
    let task_id = execution_context.task_id.clone();
    let execution_step_repo = Arc::clone(&execution_context.execution_step_repo);

    match strategy.execute(execution_context, ai_messages).await {
        Ok(result) => {
            tracing::info!(
                text_len = result.accumulated_text.len(),
                tool_call_count = result.tool_calls.len(),
                tool_result_count = result.tool_results.len(),
                "Processing complete"
            );
            Ok(result)
        },
        Err(e) => {
            tracing::error!(error = %e, "Execution failed");
            let tracking = crate::services::ExecutionTrackingService::new(execution_step_repo);
            if let Err(fail_err) = tracking
                .fail_in_progress_steps(&task_id, &e.to_string())
                .await
            {
                tracing::error!(error = %fail_err, "Failed to mark steps as failed");
            }
            Err(e)
        },
    }
}

pub(super) fn build_artifacts(
    execution_result: &ExecutionResult,
    context_id: &systemprompt_identifiers::ContextId,
    task_id: &systemprompt_identifiers::TaskId,
) -> Result<Vec<Artifact>> {
    build_artifacts_from_results(
        &execution_result.tool_results,
        &execution_result.tool_calls,
        &execution_result.tools,
        context_id,
        task_id,
    )
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to build artifacts from tool results");
        e
    })
}
