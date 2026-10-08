//! Sequential tool-execution primitives shared by the strategies.
//!
//! [`ToolExecutorTrait`] abstracts a single tool call; [`execute_tools`] runs a
//! planned batch, resolving inter-tool argument templates against the results
//! gathered so far, and the conversion helpers turn the resulting
//! [`ExecutionState`] into A2A tool calls and results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use async_trait::async_trait;
use rmcp::model::ContentBlock;
use serde_json::Value;
use std::time::Instant;

use systemprompt_identifiers::{AiToolCallId, McpToolName};
use systemprompt_models::ai::{
    ExecutionState, PlannedToolCall, PlannedToolResult, TemplateResolver,
};
use systemprompt_models::{McpTool, RequestContext, ToolCall};

pub type CallToolResult = rmcp::model::CallToolResult;

/// `#[async_trait]` is required: `execute_tools` takes the
/// executor as `&dyn ToolExecutorTrait`, so the trait must be `dyn`-compatible.
#[async_trait]
pub trait ToolExecutorTrait: Send + Sync {
    // JSON: MCP-protocol boundary — schema-less tool arguments mandated by the
    // spec.
    async fn execute_tool(
        &self,
        tool_name: &McpToolName,
        arguments: Value,
        tools: &[McpTool],
        ctx: &RequestContext,
    ) -> Result<ToolOutcome>;
}

/// A completed tool call: the structured output plus the wire `_meta` the
/// artifact transformer needs to identify the stored artifact.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    // JSON: MCP tool result — structured content is schema-less per the spec.
    pub output: Value,
    // JSON: MCP result `_meta` — the spec types it as an open object.
    pub meta: Option<Value>,
}

pub async fn execute_tools(
    calls: &[PlannedToolCall],
    tools: &[McpTool],
    ctx: &RequestContext,
    tool_executor: &dyn ToolExecutorTrait,
) -> Result<ExecutionState> {
    let mut state = ExecutionState::new();
    let total = calls.len();

    tracing::info!(tool_count = total, "Starting execution of tool calls");

    for (index, call) in calls.iter().enumerate() {
        let start = Instant::now();

        let resolved_arguments = resolve_call_arguments(call, &state);

        tracing::info!(
            index = index + 1,
            total = total,
            tool_name = %call.tool_name,
            "Executing tool"
        );

        let result = match McpToolName::try_new(call.tool_name.as_str()) {
            Ok(tool_name) => {
                tool_executor
                    .execute_tool(&tool_name, resolved_arguments.clone(), tools, ctx)
                    .await
            },
            Err(e) => Err(AgentServiceError::validation("tool_name", e)),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        state.add_result(finish_tool_call(
            call,
            resolved_arguments,
            result,
            duration_ms,
        ));

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

// JSON: MCP-protocol boundary — schema-less tool arguments mandated by the
// spec.
fn resolve_call_arguments(call: &PlannedToolCall, state: &ExecutionState) -> Value {
    let resolved_arguments = TemplateResolver::resolve_arguments(&call.arguments, &state.results);

    if call.arguments != resolved_arguments {
        tracing::info!(
            tool_name = %call.tool_name,
            original = %serde_json::to_string(&call.arguments).unwrap_or_else(|_| String::new()),
            resolved = %serde_json::to_string(&resolved_arguments).unwrap_or_else(|_| String::new()),
            "Resolved templates for tool"
        );
    }

    resolved_arguments
}

// JSON: MCP-protocol boundary — schema-less tool arguments mandated by the
// spec.
fn finish_tool_call(
    call: &PlannedToolCall,
    arguments: Value,
    result: Result<ToolOutcome>,
    duration_ms: u64,
) -> PlannedToolResult {
    let tool_name = call.tool_name.as_str();
    match result {
        Ok(outcome) => {
            tracing::info!(
                tool_name = %tool_name,
                duration_ms = duration_ms,
                "Tool completed successfully"
            );

            PlannedToolResult::success(tool_name.to_owned(), arguments, outcome.output, duration_ms)
                .with_meta(outcome.meta)
        },
        Err(e) => {
            let error_msg = e.to_string();
            tracing::error!(
                tool_name = %tool_name,
                duration_ms = duration_ms,
                error = %error_msg,
                "Tool failed"
            );

            PlannedToolResult::failure(tool_name.to_owned(), arguments, error_msg, duration_ms)
        },
    }
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
                    serde_json::to_string_pretty(&r.output).unwrap_or_else(|_| "{}".to_owned())
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
        .map(|c| ToolCall {
            ai_tool_call_id: AiToolCallId::generate(),
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
                serde_json::to_string(&r.output).unwrap_or_else(|_| "{}".to_owned())
            } else {
                r.error.clone().unwrap_or_else(|| "Error".to_owned())
            };

            let mut result = if r.success {
                CallToolResult::success(vec![ContentBlock::text(text_content)])
            } else {
                CallToolResult::error(vec![ContentBlock::text(text_content)])
            };
            result.structured_content = Some(r.output.clone());
            result.meta = r.meta.as_ref().and_then(|m| {
                serde_json::from_value(m.clone())
                    .inspect_err(|e| tracing::warn!(error = %e, "Failed to decode tool meta"))
                    .ok()
            });
            result
        })
        .collect()
}
