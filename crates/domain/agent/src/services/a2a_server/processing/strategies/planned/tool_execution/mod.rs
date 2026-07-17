//! Tool-call handling for the planned agentic strategy.
//!
//! [`handle_tool_calls`] validates the plan's argument templates, executes the
//! tools, records execution status (see [`recording`]), and synthesizes the
//! final response; validation failures are funneled back through the model for
//! a user-facing explanation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod recording;

use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::TaskId;
use systemprompt_models::ai::{
    ExecutionState, GenerateResponseParams, PlanValidationError, PlannedToolCall, TemplateValidator,
};
use systemprompt_models::{AiMessage, ExecutionStep, McpTool, PlannedTool, TrackedStep};

use super::super::plan_executor::{
    convert_to_call_tool_results, convert_to_tool_calls, execute_tools_with_templates,
    format_results_for_response,
};
use super::super::tool_executor::ContextToolExecutor;
use super::super::{ExecutionContext, ExecutionResult};
use crate::services::ExecutionTrackingService;
use crate::services::a2a_server::processing::message::StreamEvent;
use recording::{build_tool_summary, record_execution_status};

pub(super) struct HandleToolCallsParams<'a> {
    pub reasoning: String,
    pub calls: Vec<PlannedToolCall>,
    pub context: &'a ExecutionContext,
    pub tracking: &'a ExecutionTrackingService,
    pub planning_tracked: std::result::Result<(TrackedStep, ExecutionStep), AgentServiceError>,
    pub task_id: TaskId,
    pub messages: Vec<AiMessage>,
    pub tools: Vec<McpTool>,
}

pub(super) async fn handle_tool_calls(
    params: HandleToolCallsParams<'_>,
) -> Result<ExecutionResult> {
    let HandleToolCallsParams {
        reasoning,
        calls,
        context,
        tracking,
        planning_tracked,
        task_id,
        messages,
        tools,
    } = params;
    tracing::info!(
        tool_count = calls.len(),
        reasoning = %reasoning,
        "Tool calls planned"
    );

    emit_planning_complete(tracking, planning_tracked, reasoning, &calls, context).await;

    let tool_output_schemas = TemplateValidator::get_tool_output_schemas(&calls, &tools);
    if let Err(validation_errors) = TemplateValidator::validate_plan(&calls, &tool_output_schemas) {
        return handle_validation_failure(validation_errors, context, messages).await;
    }
    tracing::info!("Template validation passed");

    let (tool_name, tool_arguments) = build_tool_summary(&calls);
    let (tracked, step) = tracking
        .track_tool_execution(task_id.clone(), tool_name, tool_arguments)
        .await?;
    emit(context, StreamEvent::ExecutionStepUpdate { step });

    let tool_executor = ContextToolExecutor {
        context: context.clone(),
    };
    let state =
        execute_tools_with_templates(&calls, &tools, &context.request_ctx, &tool_executor).await?;
    let execution_summary = format_results_for_response(&state);
    let has_failures = !state.failed_results().is_empty();
    record_execution_status(tracking, &tracked, &state, has_failures).await;

    tracing::info!(
        succeeded = state.successful_results().len(),
        failed = state.failed_results().len(),
        "Execution complete"
    );

    if let Ok(step) = tracking.track_completion(task_id).await {
        emit(context, StreamEvent::ExecutionStepUpdate { step });
    }

    let tool_error_message = join_failure_errors(&state, has_failures);

    let response =
        synthesize_response(context, messages, &execution_summary, tool_error_message).await?;
    emit(context, StreamEvent::Text(response.clone()));

    let tool_calls = convert_to_tool_calls(&calls);
    let tool_results = convert_to_call_tool_results(&state);

    Ok(ExecutionResult {
        accumulated_text: response,
        tool_calls,
        tool_results,
        tools,
        iterations: 1,
    })
}

fn emit(context: &ExecutionContext, event: StreamEvent) {
    if context.tx.try_send(event).is_err() {
        tracing::debug!("Stream receiver dropped");
    }
}

fn join_failure_errors(state: &ExecutionState, has_failures: bool) -> Option<String> {
    has_failures.then(|| {
        state
            .failed_results()
            .iter()
            .filter_map(|r| r.error.as_ref())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

async fn emit_planning_complete(
    tracking: &ExecutionTrackingService,
    planning_tracked: std::result::Result<(TrackedStep, ExecutionStep), AgentServiceError>,
    reasoning: String,
    calls: &[PlannedToolCall],
    context: &ExecutionContext,
) {
    let planned_tools: Vec<PlannedTool> = calls
        .iter()
        .map(|c| PlannedTool {
            tool_name: c.tool_name.clone(),
            arguments: c.arguments.clone(),
        })
        .collect();

    if let Ok((tracked, _)) = planning_tracked
        && let Ok(step) = tracking
            .complete_planning(tracked, Some(reasoning), Some(planned_tools))
            .await
    {
        emit(context, StreamEvent::ExecutionStepUpdate { step });
    }
}

async fn synthesize_response(
    context: &ExecutionContext,
    messages: Vec<AiMessage>,
    execution_summary: &str,
    tool_error_message: Option<String>,
) -> Result<String> {
    match context
        .ai_service
        .generate_response(GenerateResponseParams {
            messages,
            execution_summary,
            context: &context.request_ctx,
            provider: context.agent_runtime.provider.as_deref(),
            model: context.agent_runtime.model.as_deref(),
            max_output_tokens: context.agent_runtime.max_output_tokens,
        })
        .await
    {
        Ok(response) => Ok(response),
        Err(ai_error) => {
            if let Some(tool_err) = tool_error_message {
                tracing::warn!(
                    ai_error = %ai_error,
                    tool_error = %tool_err,
                    "AI synthesis failed after tool errors - returning tool errors"
                );
                return Err(AgentServiceError::Internal(format!(
                    "Tool execution failed: {tool_err}"
                )));
            }
            Err(AgentServiceError::Internal(format!("{ai_error}")))
        },
    }
}

async fn handle_validation_failure(
    validation_errors: Vec<PlanValidationError>,
    context: &ExecutionContext,
    messages: Vec<AiMessage>,
) -> Result<ExecutionResult> {
    let error_messages: Vec<String> = validation_errors.iter().map(ToString::to_string).collect();

    tracing::error!(
        errors = ?error_messages,
        "Template validation failed"
    );

    let validation_summary = format!(
        "Plan validation failed:\n{}",
        error_messages
            .iter()
            .map(|e| format!("- {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let response = context
        .ai_service
        .generate_response(GenerateResponseParams {
            messages,
            execution_summary: &validation_summary,
            context: &context.request_ctx,
            provider: context.agent_runtime.provider.as_deref(),
            model: context.agent_runtime.model.as_deref(),
            max_output_tokens: context.agent_runtime.max_output_tokens,
        })
        .await?;

    emit(context, StreamEvent::Text(response.clone()));

    Ok(ExecutionResult {
        accumulated_text: response,
        tool_calls: vec![],
        tool_results: vec![],
        tools: vec![],
        iterations: 1,
    })
}
