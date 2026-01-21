use anyhow::Result;
use serde_json::Value;
use systemprompt_identifiers::TaskId;
use systemprompt_models::ai::{GenerateResponseParams, PlannedCall, TemplateValidator};
use systemprompt_models::{AiMessage, ExecutionStep, McpTool, PlannedTool, TrackedStep};

use super::super::plan_executor::{
    convert_to_call_tool_results, convert_to_tool_calls, execute_tools_with_templates,
    format_results_for_response,
};
use super::super::tool_executor::ContextToolExecutor;
use super::super::{ExecutionContext, ExecutionResult};
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::ExecutionTrackingService;

pub async fn handle_tool_calls(
    reasoning: String,
    calls: Vec<PlannedCall>,
    context: &ExecutionContext,
    tracking: &ExecutionTrackingService,
    planning_tracked: Result<(TrackedStep, ExecutionStep), anyhow::Error>,
    task_id: TaskId,
    messages: Vec<AiMessage>,
    tools: Vec<McpTool>,
) -> Result<ExecutionResult> {
    tracing::info!(
        tool_count = calls.len(),
        reasoning = %reasoning,
        "Tool calls planned"
    );

    let planned_tools: Vec<PlannedTool> = calls
        .iter()
        .map(|c| PlannedTool {
            tool_name: c.tool_name.clone(),
            arguments: c.arguments.clone(),
        })
        .collect();

    if let Ok((tracked, _)) = planning_tracked {
        if let Ok(step) = tracking
            .complete_planning(tracked, Some(reasoning.clone()), Some(planned_tools))
            .await
        {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped");
            }
        }
    }

    let tool_output_schemas = TemplateValidator::get_tool_output_schemas(&calls, &tools);

    if let Err(validation_errors) = TemplateValidator::validate_plan(&calls, &tool_output_schemas) {
        return handle_validation_failure(validation_errors, context, messages).await;
    }

    tracing::info!("Template validation passed");

    let (tool_name, tool_arguments) = build_tool_summary(&calls);

    let (tracked, step) = tracking
        .track_tool_execution(task_id.clone(), tool_name, tool_arguments)
        .await?;

    if context
        .tx
        .send(StreamEvent::ExecutionStepUpdate { step })
        .is_err()
    {
        tracing::debug!("Stream receiver dropped");
    }

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
        if context
            .tx
            .send(StreamEvent::ExecutionStepUpdate { step })
            .is_err()
        {
            tracing::debug!("Stream receiver dropped");
        }
    }

    let tool_error_message: Option<String> = if has_failures {
        Some(
            state
                .failed_results()
                .iter()
                .filter_map(|r| r.error.as_ref())
                .map(|e| e.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    } else {
        None
    };

    let response = match context
        .ai_service
        .generate_response(GenerateResponseParams {
            messages,
            execution_summary: &execution_summary,
            context: &context.request_ctx,
            provider: context.agent_runtime.provider.as_deref(),
            model: context.agent_runtime.model.as_deref(),
            max_output_tokens: context.agent_runtime.max_output_tokens,
        })
        .await
    {
        Ok(response) => response,
        Err(ai_error) => {
            if let Some(tool_err) = tool_error_message {
                tracing::warn!(
                    ai_error = %ai_error,
                    tool_error = %tool_err,
                    "AI synthesis failed after tool errors - returning tool errors"
                );
                return Err(anyhow::anyhow!("Tool execution failed: {}", tool_err));
            }
            return Err(ai_error);
        },
    };

    if context
        .tx
        .send(StreamEvent::Text(response.clone()))
        .is_err()
    {
        tracing::debug!("Stream receiver dropped");
    }

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

async fn handle_validation_failure(
    validation_errors: Vec<systemprompt_models::ai::TemplateValidationError>,
    context: &ExecutionContext,
    messages: Vec<AiMessage>,
) -> Result<ExecutionResult> {
    let error_messages: Vec<String> = validation_errors.iter().map(|e| e.to_string()).collect();

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

    if context
        .tx
        .send(StreamEvent::Text(response.clone()))
        .is_err()
    {
        tracing::debug!("Stream receiver dropped");
    }

    Ok(ExecutionResult {
        accumulated_text: response,
        tool_calls: vec![],
        tool_results: vec![],
        tools: vec![],
        iterations: 1,
    })
}

fn build_tool_summary(calls: &[PlannedCall]) -> (String, Value) {
    if calls.len() == 1 {
        (calls[0].tool_name.clone(), calls[0].arguments.clone())
    } else {
        let tool_args_summary: Vec<Value> = calls
            .iter()
            .map(|c| {
                serde_json::json!({
                    "tool": c.tool_name,
                    "arguments": c.arguments
                })
            })
            .collect();
        (
            format!("{} tools", calls.len()),
            serde_json::json!(tool_args_summary),
        )
    }
}

async fn record_execution_status(
    tracking: &ExecutionTrackingService,
    tracked: &TrackedStep,
    state: &super::super::plan_executor::ExecutionState,
    has_failures: bool,
) {
    if has_failures {
        let error_message = state
            .failed_results()
            .iter()
            .filter_map(|r| r.error.as_ref())
            .map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        if let Err(e) = tracking.fail(tracked, error_message).await {
            tracing::warn!(error = %e, "Failed to record execution failure");
        }
    } else {
        let tool_result = if state.results.len() == 1 {
            serde_json::json!({
                "tool": state.results[0].tool_name,
                "output": state.results[0].output,
                "duration_ms": state.results[0].duration_ms
            })
        } else {
            serde_json::json!({
                "results": state.results.iter().map(|r| {
                    serde_json::json!({
                        "tool": r.tool_name,
                        "output": r.output,
                        "duration_ms": r.duration_ms
                    })
                }).collect::<Vec<_>>()
            })
        };

        if let Err(e) = tracking.complete(tracked.clone(), Some(tool_result)).await {
            tracing::warn!(error = %e, "Failed to record execution completion");
        }
    }
}
