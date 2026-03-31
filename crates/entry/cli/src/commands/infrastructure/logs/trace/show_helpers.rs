use anyhow::Result;
use systemprompt_logging::{AiTraceService, CliService, TraceEvent};

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_execution_steps, print_task_info,
    print_user_input,
};
use super::ai_mcp::print_mcp_executions;
use super::show::ShowArgs;
use super::{AiSummaryRow, McpSummaryRow, StepSummaryRow, TraceEventRow, TraceViewOutput};
use crate::shared::CommandResult;

pub(super) struct TraceSummaries<'a> {
    pub ai: &'a systemprompt_logging::AiRequestSummary,
    pub mcp: &'a systemprompt_logging::McpExecutionSummary,
    pub step: &'a systemprompt_logging::ExecutionStepSummary,
}

pub(super) async fn execute_ai_trace(
    service: &AiTraceService,
    task_id: &str,
    args: &ShowArgs,
) -> Result<CommandResult<TraceViewOutput>> {
    if !args.json {
        CliService::section(&format!("Trace: {}", task_id));
    }

    let task_info = service.get_task_info(task_id).await?;
    let context_id = task_info.context_id.clone();

    if !args.json {
        print_task_info(&task_info);
    }

    let user_input = service.get_user_input(task_id).await?;
    if !args.json {
        print_user_input(user_input.as_ref());
    }

    let show_all = args.all;

    if show_all || args.steps {
        let steps = service.get_execution_steps(task_id).await?;
        if !args.json {
            print_execution_steps(&steps);
        }
    }

    if show_all || args.ai {
        let ai_requests = service.get_ai_requests(task_id).await?;
        if !args.json {
            print_ai_requests(&ai_requests);
        }
    }

    if show_all || args.mcp {
        let mcp_executions = service.get_mcp_executions(task_id, &context_id).await?;
        if !args.json {
            print_mcp_executions(service, &mcp_executions, task_id, &context_id, args.verbose)
                .await;
        }
    }

    if show_all || args.artifacts {
        let artifacts = service.get_task_artifacts(task_id, &context_id).await?;
        if !args.json {
            print_artifacts(&artifacts);
        }
    }

    let response = service.get_agent_response(task_id).await?;
    if !args.json {
        print_agent_response(response.as_ref());
        CliService::info(&"═".repeat(60));
    }

    let output = TraceViewOutput {
        trace_id: task_id.to_string(),
        events: Vec::new(),
        ai_summary: AiSummaryRow {
            request_count: 0,
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost_dollars: 0.0,
            total_latency_ms: 0,
        },
        mcp_summary: McpSummaryRow {
            execution_count: 0,
            total_execution_time_ms: 0,
        },
        step_summary: StepSummaryRow {
            total: 0,
            completed: 0,
            failed: 0,
            pending: 0,
        },
        task_id: Some(task_id.to_string()),
        duration_ms: None,
        status: task_info.status,
    };

    Ok(CommandResult::card(output)
        .with_title("AI Trace Details")
        .with_skip_render())
}

pub(super) fn build_trace_output(
    trace_id: &str,
    events: &[TraceEvent],
    summaries: &TraceSummaries<'_>,
    task_id: Option<&str>,
    duration_ms: Option<i64>,
) -> TraceViewOutput {
    let first_timestamp = events.first().map(|e| e.timestamp);

    let event_rows: Vec<TraceEventRow> = events
        .iter()
        .map(|e| {
            let delta_ms =
                first_timestamp.map_or(0, |first| (e.timestamp - first).num_milliseconds());
            TraceEventRow {
                timestamp: e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                delta_ms,
                event_type: e.event_type.clone(),
                details: e.details.clone(),
                latency_ms: None,
            }
        })
        .collect();

    let cost_dollars = f64::from(summaries.ai.total_cost_microdollars as i32) / 1_000_000.0;

    let status = if summaries.step.failed > 0 {
        "failed".to_string()
    } else if summaries.step.pending > 0 {
        "in_progress".to_string()
    } else {
        "completed".to_string()
    };

    TraceViewOutput {
        trace_id: trace_id.to_string(),
        events: event_rows,
        ai_summary: AiSummaryRow {
            request_count: summaries.ai.request_count,
            total_tokens: summaries.ai.total_input_tokens + summaries.ai.total_output_tokens,
            input_tokens: summaries.ai.total_input_tokens,
            output_tokens: summaries.ai.total_output_tokens,
            cost_dollars,
            total_latency_ms: summaries.ai.total_latency_ms,
        },
        mcp_summary: McpSummaryRow {
            execution_count: summaries.mcp.execution_count,
            total_execution_time_ms: summaries.mcp.total_execution_time_ms,
        },
        step_summary: StepSummaryRow {
            total: summaries.step.total,
            completed: summaries.step.completed,
            failed: summaries.step.failed,
            pending: summaries.step.pending,
        },
        task_id: task_id.map(String::from),
        duration_ms,
        status,
    }
}

pub(super) fn filter_log_events(log_events: Vec<TraceEvent>, verbose: bool) -> Vec<TraceEvent> {
    if verbose {
        return log_events;
    }

    log_events
        .into_iter()
        .filter(|e| {
            let event_type = e.event_type.to_lowercase();
            let details = e.details.to_lowercase();

            if event_type == "debug" {
                return false;
            }

            if event_type == "warn" || event_type == "error" {
                return true;
            }

            if details.contains("broadcast execution_step")
                || details.contains("resolved redirect")
                || details.contains("resolving redirect")
                || details.contains("artifacts count")
                || details.contains("task.artifacts")
                || details.contains("sending json")
                || details.contains("received artifact")
                || details.contains("received complete event")
                || details.contains("setting task.artifacts")
                || details.contains("sent complete event")
            {
                return false;
            }

            if details.contains("🚀 starting")
                || details.contains("agentic loop complete")
                || details.contains("loop finished")
                || details.contains("processing complete")
                || details.contains("ai request")
                || details.contains("tool executed")
                || details.contains("iteration") && details.contains("starting")
                || details.contains("decision:")
                || details.contains("research complete")
                || details.contains("synthesis complete")
            {
                return true;
            }

            false
        })
        .collect()
}
