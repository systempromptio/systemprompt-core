//! AI-trace table assembly with noise filtering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_runtime::{AiTraceService, TraceEvent};

use super::ai_artifacts::artifacts as artifacts_section;
use super::ai_display::{
    Transcript, agent_response, ai_requests as ai_requests_section, execution_steps, task_info,
    user_input,
};
use super::ai_mcp::mcp_executions as mcp_executions_section;
use super::show::ShowArgs;
use super::{AiSummaryRow, McpSummaryRow, StepSummaryRow, TraceViewOutput};
use systemprompt_extension::cli::CommandOutput;

pub(super) async fn execute_ai_trace(
    service: &AiTraceService,
    task_id: &TaskId,
    args: &ShowArgs,
    machine_output: bool,
) -> Result<CommandOutput> {
    let text = !args.json && !machine_output;
    let mut out = Transcript::default();

    let info = service.get_task_info(task_id).await?;
    let context_id: ContextId = info.context_id.clone();
    task_info(&mut out, &info);

    let input = service.get_user_input(task_id).await?;
    user_input(&mut out, input.as_ref());

    let show_all = args.sections.all;

    let steps = service.get_execution_steps(task_id).await?;
    if show_all || args.sections.steps {
        execution_steps(&mut out, &steps);
    }

    let ai_requests = service.get_ai_requests(task_id).await?;
    if show_all || args.sections.ai {
        ai_requests_section(&mut out, &ai_requests);
    }

    let mcp_executions = service.get_mcp_executions(task_id, &context_id).await?;
    if text && (show_all || args.sections.mcp) {
        mcp_executions_section(
            &mut out,
            service,
            &mcp_executions,
            task_id,
            &context_id,
            args.verbose,
        )
        .await;
    }

    if text && (show_all || args.sections.artifacts) {
        let artifacts = service.get_task_artifacts(task_id, &context_id).await?;
        artifacts_section(&mut out, &artifacts);
    }

    let response = service.get_agent_response(task_id).await?;
    agent_response(&mut out, response.as_ref());

    let output = build_trace_output(task_id, &info, &ai_requests, &mcp_executions, &steps);

    if args.json {
        let content =
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| format!("{output:?}"));
        return Ok(CommandOutput::copy_paste_titled("Trace JSON", content));
    }
    if machine_output {
        return Ok(CommandOutput::card_value("AI Trace Details", &output));
    }

    Ok(out.into_output(format!("Trace: {}", task_id.as_str())))
}

fn ai_summary(ai_requests: &[systemprompt_runtime::AiRequestInfo]) -> AiSummaryRow {
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
    task_info: &systemprompt_runtime::TaskInfo,
    ai_requests: &[systemprompt_runtime::AiRequestInfo],
    mcp_executions: &[systemprompt_runtime::McpToolExecution],
    steps: &[systemprompt_runtime::ExecutionStep],
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
