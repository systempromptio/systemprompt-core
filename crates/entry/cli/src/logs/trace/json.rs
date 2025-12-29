use anyhow::Result;
use serde_json::Value;
use systemprompt_core_logging::{
    AiRequestSummary, CliService, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
};

pub fn print_json(
    events: &[TraceEvent],
    trace_id: &str,
    ai_summary: &AiRequestSummary,
    mcp_summary: &McpExecutionSummary,
    step_summary: &ExecutionStepSummary,
) -> Result<()> {
    let json_events: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut obj = serde_json::Map::new();
            obj.insert("type".to_string(), Value::String(e.event_type.clone()));
            obj.insert(
                "timestamp".to_string(),
                Value::String(e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
            );
            obj.insert("details".to_string(), Value::String(e.details.clone()));

            if let Some(ref user_id) = e.user_id {
                obj.insert("user_id".to_string(), Value::String(user_id.clone()));
            }
            if let Some(ref session_id) = e.session_id {
                obj.insert("session_id".to_string(), Value::String(session_id.clone()));
            }
            if let Some(ref task_id) = e.task_id {
                obj.insert("task_id".to_string(), Value::String(task_id.clone()));
            }
            if let Some(ref context_id) = e.context_id {
                obj.insert("context_id".to_string(), Value::String(context_id.clone()));
            }
            if let Some(ref metadata) = e.metadata {
                if let Ok(parsed) = serde_json::from_str::<Value>(metadata) {
                    obj.insert("metadata".to_string(), parsed);
                }
            }

            Value::Object(obj)
        })
        .collect();

    let output = serde_json::json!({
        "trace_id": trace_id,
        "events": json_events,
        "count": events.len(),
        "ai_summary": {
            "request_count": ai_summary.request_count,
            "total_tokens": ai_summary.total_tokens,
            "input_tokens": ai_summary.total_input_tokens,
            "output_tokens": ai_summary.total_output_tokens,
            "cost_cents": ai_summary.total_cost_cents,
            "total_latency_ms": ai_summary.total_latency_ms,
        },
        "mcp_summary": {
            "execution_count": mcp_summary.execution_count,
            "total_execution_time_ms": mcp_summary.total_execution_time_ms,
        },
        "step_summary": {
            "step_count": step_summary.total,
            "completed_count": step_summary.completed,
            "failed_count": step_summary.failed,
            "pending_count": step_summary.pending,
        }
    });

    CliService::json(&output);
    Ok(())
}
