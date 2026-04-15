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
use crate::shared::CommandResult;

pub async fn execute_ai_trace(
    service: &AiTraceService,
    task_id: &TaskId,
    args: &ShowArgs,
) -> Result<CommandResult<TraceViewOutput>> {
    if !args.json {
        CliService::section(&format!("Trace: {}", task_id.as_str()));
    }

    let task_info = service.get_task_info(task_id.as_str()).await?;
    let context_id: ContextId = task_info.context_id.clone();

    if !args.json {
        print_task_info(&task_info);
    }

    let user_input = service.get_user_input(task_id.as_str()).await?;
    if !args.json {
        print_user_input(user_input.as_ref());
    }

    let show_all = args.sections.all;

    let steps = service.get_execution_steps(task_id.as_str()).await?;
    if (show_all || args.sections.steps) && !args.json {
        print_execution_steps(&steps);
    }

    let ai_requests = service.get_ai_requests(task_id.as_str()).await?;
    if (show_all || args.sections.ai) && !args.json {
        print_ai_requests(&ai_requests);
    }

    let mcp_executions = service
        .get_mcp_executions(task_id.as_str(), context_id.as_str())
        .await?;
    if (show_all || args.sections.mcp) && !args.json {
        print_mcp_executions(service, &mcp_executions, task_id, &context_id, args.verbose).await;
    }

    if show_all || args.sections.artifacts {
        let artifacts = service
            .get_task_artifacts(task_id.as_str(), context_id.as_str())
            .await?;
        if !args.json {
            print_artifacts(&artifacts);
        }
    }

    let response = service.get_agent_response(task_id.as_str()).await?;
    if !args.json {
        print_agent_response(response.as_ref());
        CliService::info(&"═".repeat(60));
    }

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

    let duration_ms = task_info
        .started_at
        .zip(task_info.completed_at)
        .map(|(s, e)| (e - s).num_milliseconds());

    let output = TraceViewOutput {
        trace_id: systemprompt_identifiers::TraceId::new(task_id.as_str()),
        events: Vec::new(),
        ai_summary: AiSummaryRow {
            request_count: ai_requests.len() as i64,
            total_tokens: total_input_tokens + total_output_tokens,
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            cost_dollars: total_cost_microdollars as f64 / 1_000_000.0,
            total_latency_ms,
        },
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
        task: Some(task_id.as_str().to_string()),
        duration_ms,
        status: task_info.status,
    };

    Ok(CommandResult::card(output)
        .with_title("AI Trace Details")
        .with_skip_render())
}

pub fn filter_log_events(log_events: Vec<TraceEvent>, verbose: bool) -> Vec<TraceEvent> {
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
