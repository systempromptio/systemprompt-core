//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_logging::{AiTraceService, CliService, TraceEvent};

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_execution_steps, print_task_info,
    print_user_input,
};
use super::ai_mcp::print_mcp_executions;
use super::show::ShowArgs;
use super::{AiSummaryRow, McpSummaryRow, StepSummaryRow, TraceViewOutput};
use crate::shared::CommandOutput;

pub(super) async fn execute_ai_trace(
    service: &AiTraceService,
    task_id: &TaskId,
    args: &ShowArgs,
) -> Result<CommandOutput> {
    if !args.json {
        CliService::section(&format!("Trace: {}", task_id.as_str()));
    }

    let task_info = service.get_task_info(task_id).await?;
    let context_id: ContextId = task_info.context_id.clone();

    if !args.json {
        print_task_info(&task_info);
    }

    let user_input = service.get_user_input(task_id).await?;
    if !args.json {
        print_user_input(user_input.as_ref());
    }

    let show_all = args.sections.all;

    let steps = service.get_execution_steps(task_id).await?;
    if (show_all || args.sections.steps) && !args.json {
        print_execution_steps(&steps);
    }

    let ai_requests = service.get_ai_requests(task_id).await?;
    if (show_all || args.sections.ai) && !args.json {
        print_ai_requests(&ai_requests);
    }

    let mcp_executions = service.get_mcp_executions(task_id, &context_id).await?;
    if (show_all || args.sections.mcp) && !args.json {
        print_mcp_executions(service, &mcp_executions, task_id, &context_id, args.verbose).await;
    }

    if show_all || args.sections.artifacts {
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

    let output = build_trace_output(task_id, &task_info, &ai_requests, &mcp_executions, &steps);

    if args.json {
        let content =
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| format!("{output:?}"));
        return Ok(CommandOutput::copy_paste_titled("Trace JSON", content));
    }

    Ok(CommandOutput::card_value("AI Trace Details", &output).with_skip_render())
}

fn ai_summary(ai_requests: &[systemprompt_logging::AiRequestInfo]) -> AiSummaryRow {
    let total_input_tokens: i64 = ai_requests
        .iter()
        .map(|r| i64::from(r.input_tokens.unwrap_or(0)))
        .sum();
    let total_output_tokens: i64 = ai_requests
        .iter()
        .map(|r| i64::from(r.output_tokens.unwrap_or(0)))
        .sum();
    let total_cost_microdollars: i64 = ai_requests.iter().map(|r| r.cost_microdollars).sum();
    let total_latency_ms: i64 = ai_requests
        .iter()
        .map(|r| i64::from(r.latency_ms.unwrap_or(0)))
        .sum();

    AiSummaryRow {
        request_count: ai_requests.len() as i64,
        total_tokens: total_input_tokens + total_output_tokens,
        input_tokens: total_input_tokens,
        output_tokens: total_output_tokens,
        cost_dollars: total_cost_microdollars as f64 / 1_000_000.0,
        total_latency_ms,
    }
}

fn build_trace_output(
    task_id: &TaskId,
    task_info: &systemprompt_logging::TaskInfo,
    ai_requests: &[systemprompt_logging::AiRequestInfo],
    mcp_executions: &[systemprompt_logging::McpToolExecution],
    steps: &[systemprompt_logging::ExecutionStep],
) -> TraceViewOutput {
    let duration_ms = task_info
        .started_at
        .zip(task_info.completed_at)
        .map(|(s, e)| (e - s).num_milliseconds());

    TraceViewOutput {
        trace_id: systemprompt_identifiers::TraceId::new(task_id.as_str()),
        events: Vec::new(),
        ai_summary: ai_summary(ai_requests),
        mcp_summary: McpSummaryRow {
            execution_count: mcp_executions.len() as i64,
            total_execution_time_ms: mcp_executions
                .iter()
                .map(|e| i64::from(e.execution_time_ms.unwrap_or(0)))
                .sum(),
        },
        step_summary: StepSummaryRow {
            total: steps.len() as i64,
            completed: steps.iter().filter(|s| s.status == "completed").count() as i64,
            failed: steps.iter().filter(|s| s.status == "failed").count() as i64,
            pending: steps
                .iter()
                .filter(|s| s.status == "pending" || s.status == "in_progress")
                .count() as i64,
        },
        task: Some(task_id.as_str().to_owned()),
        duration_ms,
        status: task_info.status.clone(),
    }
}

const NOISE_DETAILS: &[&str] = &[
    "broadcast execution_step",
    "resolved redirect",
    "resolving redirect",
    "artifacts count",
    "task.artifacts",
    "sending json",
    "received artifact",
    "received complete event",
    "setting task.artifacts",
    "sent complete event",
];

const SIGNAL_DETAILS: &[&str] = &[
    "🚀 starting",
    "agentic loop complete",
    "loop finished",
    "processing complete",
    "ai request",
    "tool executed",
    "decision:",
    "research complete",
    "synthesis complete",
];

fn keep_event(event_type: &str, details: &str) -> bool {
    if event_type == "debug" {
        return false;
    }
    if matches!(event_type, "warn" | "error") {
        return true;
    }
    if NOISE_DETAILS.iter().any(|n| details.contains(n)) {
        return false;
    }
    if SIGNAL_DETAILS.iter().any(|s| details.contains(s)) {
        return true;
    }
    details.contains("iteration") && details.contains("starting")
}

pub(super) fn filter_log_events(log_events: Vec<TraceEvent>, verbose: bool) -> Vec<TraceEvent> {
    if verbose {
        return log_events;
    }
    log_events
        .into_iter()
        .filter(|e| keep_event(&e.event_type.to_lowercase(), &e.details.to_lowercase()))
        .collect()
}
