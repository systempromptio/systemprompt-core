use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::Content;
use serde_json::Value;
use std::time::Instant;

use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::ai::{ExecutionState, PlannedToolCall, TemplateResolver, ToolCallResult};
use systemprompt_models::{McpTool, RequestContext, ToolCall};

pub type CallToolResult = rmcp::model::CallToolResult;

#[async_trait]
pub trait ToolExecutorTrait: Send + Sync {
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        tools: &[McpTool],
        ctx: &RequestContext,
    ) -> Result<Value>;
}

pub async fn execute_tools_sequentially(
    calls: &[PlannedToolCall],
    tools: &[McpTool],
    ctx: &RequestContext,
    tool_executor: &dyn ToolExecutorTrait,
) -> Result<ExecutionState> {
    let mut state = ExecutionState::new();
    let total = calls.len();

    tracing::info!(
        tool_count = total,
        "Starting sequential execution of tool calls"
    );

    for (index, call) in calls.iter().enumerate() {
        let start = Instant::now();

        tracing::info!(
            index = index + 1,
            total = total,
            tool_name = %call.tool_name,
            "Executing tool"
        );

        let result = tool_executor
            .execute_tool(&call.tool_name, call.arguments.clone(), tools, ctx)
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let tool_result = match result {
            Ok(output) => {
                tracing::info!(
                    tool_name = %call.tool_name,
                    duration_ms = duration_ms,
                    "Tool completed successfully"
                );

                ToolCallResult::success(
                    call.tool_name.clone(),
                    call.arguments.clone(),
                    output,
                    duration_ms,
                )
            },
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!(
                    tool_name = %call.tool_name,
                    duration_ms = duration_ms,
                    error = %error_msg,
                    "Tool failed"
                );

                ToolCallResult::failure(
                    call.tool_name.clone(),
                    call.arguments.clone(),
                    error_msg,
                    duration_ms,
                )
            },
        };

        state.add_result(tool_result);

        if state.halted {
            tracing::warn!(
                index = index + 1,
                total = total,
                reason = state.halt_reason.as_deref().unwrap_or("Unknown"),
                "Execution halted"
            );
            break;
        }
    }

    tracing::info!(
        successful = state.successful_results().len(),
        failed = state.failed_results().len(),
        total_duration_ms = state.total_duration_ms(),
        "Execution complete"
    );

    Ok(state)
}

pub async fn execute_tools_with_templates(
    calls: &[PlannedToolCall],
    tools: &[McpTool],
    ctx: &RequestContext,
    tool_executor: &dyn ToolExecutorTrait,
) -> Result<ExecutionState> {
    let mut state = ExecutionState::new();
    let total = calls.len();

    tracing::info!(
        tool_count = total,
        "Starting template-aware execution of tool calls"
    );

    for (index, call) in calls.iter().enumerate() {
        let start = Instant::now();

        let resolved_arguments =
            TemplateResolver::resolve_arguments(&call.arguments, &state.results);

        let has_templates = call.arguments != resolved_arguments;
        if has_templates {
            tracing::info!(
                tool_name = %call.tool_name,
                original = %serde_json::to_string(&call.arguments).unwrap_or_default(),
                resolved = %serde_json::to_string(&resolved_arguments).unwrap_or_default(),
                "Resolved templates for tool"
            );
        }

        tracing::info!(
            index = index + 1,
            total = total,
            tool_name = %call.tool_name,
            "Executing tool"
        );

        let result = tool_executor
            .execute_tool(&call.tool_name, resolved_arguments.clone(), tools, ctx)
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let tool_result = match result {
            Ok(output) => {
                tracing::info!(
                    tool_name = %call.tool_name,
                    duration_ms = duration_ms,
                    "Tool completed successfully"
                );

                ToolCallResult::success(
                    call.tool_name.clone(),
                    resolved_arguments,
                    output,
                    duration_ms,
                )
            },
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!(
                    tool_name = %call.tool_name,
                    duration_ms = duration_ms,
                    error = %error_msg,
                    "Tool failed"
                );

                ToolCallResult::failure(
                    call.tool_name.clone(),
                    resolved_arguments,
                    error_msg,
                    duration_ms,
                )
            },
        };

        state.add_result(tool_result);

        if state.halted {
            tracing::warn!(
                index = index + 1,
                total = total,
                reason = state.halt_reason.as_deref().unwrap_or("Unknown"),
                "Execution halted"
            );
            break;
        }
    }

    tracing::info!(
        successful = state.successful_results().len(),
        failed = state.failed_results().len(),
        total_duration_ms = state.total_duration_ms(),
        "Template execution complete"
    );

    Ok(state)
}

pub fn format_results_for_response(state: &ExecutionState) -> String {
    state
        .results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.success {
                format!(
                    "{}. {} - SUCCESS\n   Result: {}",
                    i + 1,
                    r.tool_name,
                    serde_json::to_string_pretty(&r.output).unwrap_or_else(|_| "{}".to_string())
                )
            } else {
                format!(
                    "{}. {} - FAILED\n   Error: {}",
                    i + 1,
                    r.tool_name,
                    r.error.as_deref().unwrap_or("Unknown error")
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn convert_to_tool_calls(calls: &[PlannedToolCall]) -> Vec<ToolCall> {
    calls
        .iter()
        .enumerate()
        .map(|(i, c)| ToolCall {
            ai_tool_call_id: AiToolCallId::new(format!("plan_call_{i}")),
            name: c.tool_name.clone(),
            arguments: c.arguments.clone(),
        })
        .collect()
}

pub fn convert_to_call_tool_results(state: &ExecutionState) -> Vec<CallToolResult> {
    state
        .results
        .iter()
        .map(|r| {
            let text_content = if r.success {
                serde_json::to_string(&r.output).unwrap_or_else(|_| "{}".to_string())
            } else {
                r.error.clone().unwrap_or_else(|| "Error".to_string())
            };

            CallToolResult {
                content: vec![Content::text(text_content)],
                structured_content: Some(r.output.clone()),
                is_error: Some(!r.success),
                meta: None,
            }
        })
        .collect()
}
