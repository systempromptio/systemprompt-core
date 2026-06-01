//! Execution-result summarisation and tracking-status recording for the
//! planned strategy's tool calls.

use serde_json::Value;
use systemprompt_models::ai::{ExecutionState, PlannedToolCall};
use systemprompt_models::TrackedStep;

use crate::services::ExecutionTrackingService;

pub(super) fn build_tool_summary(calls: &[PlannedToolCall]) -> (String, Value) {
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

pub(super) async fn record_execution_status(
    tracking: &ExecutionTrackingService,
    tracked: &TrackedStep,
    state: &ExecutionState,
    has_failures: bool,
) {
    if has_failures {
        let error_message = state
            .failed_results()
            .iter()
            .filter_map(|r| r.error.as_ref())
            .map(String::as_str)
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
