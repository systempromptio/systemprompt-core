use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::{AiTraceService, CliService, TraceEvent, TraceQueryService};
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_execution_steps, print_task_info,
    print_user_input,
};
use super::ai_mcp::print_mcp_executions;
use super::display::{print_event, print_table};
use super::json::print_json;
use super::summary::{print_summary, SummaryContext};
use super::{AiSummaryRow, McpSummaryRow, StepSummaryRow, TraceEventRow, TraceViewOutput};
use crate::shared::CommandResult;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Trace ID or Task ID (can be partial)")]
    pub id: String,

    #[arg(long, help = "Show detailed metadata for each event")]
    pub verbose: bool,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[arg(long, help = "Show execution steps")]
    pub steps: bool,

    #[arg(long, help = "Show AI requests in trace")]
    pub ai: bool,

    #[arg(long, help = "Show MCP tool calls in trace")]
    pub mcp: bool,

    #[arg(long, help = "Show artifacts")]
    pub artifacts: bool,

    #[arg(long, help = "Show all sections (steps, ai, mcp, artifacts)")]
    pub all: bool,
}

struct TraceSummaries<'a> {
    ai: &'a systemprompt_logging::AiRequestSummary,
    mcp: &'a systemprompt_logging::McpExecutionSummary,
    step: &'a systemprompt_logging::ExecutionStepSummary,
}

struct FormattedDisplayContext<'a> {
    events: &'a [TraceEvent],
    trace_id: &'a str,
    task_id: Option<&'a str>,
    verbose: bool,
    ai_summary: &'a systemprompt_logging::AiRequestSummary,
    mcp_summary: &'a systemprompt_logging::McpExecutionSummary,
    step_summary: &'a systemprompt_logging::ExecutionStepSummary,
}

pub async fn execute(args: ShowArgs) -> Result<CommandResult<TraceViewOutput>> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
) -> Result<CommandResult<TraceViewOutput>> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool).await
}

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandResult<TraceViewOutput>> {
    let ai_service = AiTraceService::new(Arc::clone(pool));
    if let Ok(task_id) = ai_service.resolve_task_id(&args.id).await {
        return execute_ai_trace(&ai_service, &task_id, &args).await;
    }

    execute_trace_view(&args, pool).await
}

async fn execute_trace_view(
    args: &ShowArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandResult<TraceViewOutput>> {
    let service = TraceQueryService::new(Arc::clone(pool));

    let (
        log_events,
        ai_events,
        mcp_events,
        step_events,
        ai_summary,
        mcp_summary,
        step_summary,
        task_id,
    ) = service.get_all_trace_data(&args.id).await?;

    let filtered_log_events = filter_log_events(log_events, args.verbose);

    let mut events = filtered_log_events;
    events.extend(ai_events);
    events.extend(mcp_events);
    events.extend(step_events);
    events.sort_by_key(|e| e.timestamp);

    let first_timestamp = events.first().map(|e| e.timestamp);
    let last_timestamp = events.last().map(|e| e.timestamp);
    let duration_ms = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) => Some((last - first).num_milliseconds()),
        _ => None,
    };

    let summaries = TraceSummaries {
        ai: &ai_summary,
        mcp: &mcp_summary,
        step: &step_summary,
    };
    let output = build_trace_output(
        &args.id,
        &events,
        &summaries,
        task_id.as_deref(),
        duration_ms,
    );

    let result = CommandResult::card(output).with_title("Trace Details");

    if events.is_empty() {
        if ai_summary.request_count > 0 || mcp_summary.execution_count > 0 {
            if !args.json {
                CliService::section(&format!("Trace: {}", args.id));
                CliService::info("No log events found, but trace has activity:");
                if ai_summary.request_count > 0 {
                    CliService::key_value("AI Requests", &ai_summary.request_count.to_string());
                    CliService::key_value(
                        "Total Tokens",
                        &format!(
                            "{} in / {} out",
                            ai_summary.total_input_tokens, ai_summary.total_output_tokens
                        ),
                    );
                    let cost_dollars =
                        f64::from(ai_summary.total_cost_microdollars as i32) / 1_000_000.0;
                    CliService::key_value("Cost", &format!("${:.6}", cost_dollars));
                }
                if mcp_summary.execution_count > 0 {
                    CliService::key_value("MCP Calls", &mcp_summary.execution_count.to_string());
                }
                CliService::info("Use --verbose to see all log entries, or --ai/--mcp for details");
            }
            return Ok(result.with_skip_render());
        }
        if !args.json {
            CliService::warning(&format!("No events found for trace: {}", args.id));
            CliService::info(
                "Tip: The trace may take a moment to populate. Try again in a few seconds.",
            );
        }
        return Ok(result.with_skip_render());
    }

    if args.json {
        print_json(&events, &args.id, &ai_summary, &mcp_summary, &step_summary);
        return Ok(result.with_skip_render());
    }

    let display_ctx = FormattedDisplayContext {
        events: &events,
        trace_id: &args.id,
        task_id: task_id.as_deref(),
        verbose: args.verbose,
        ai_summary: &ai_summary,
        mcp_summary: &mcp_summary,
        step_summary: &step_summary,
    };
    print_formatted(&display_ctx);

    Ok(result.with_skip_render())
}

async fn execute_ai_trace(
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

fn print_formatted(ctx: &FormattedDisplayContext<'_>) {
    CliService::section(&format!("Trace Flow: {}", ctx.trace_id));

    let first_timestamp = ctx.events.first().map(|e| e.timestamp);
    let last_timestamp = ctx.events.last().map(|e| e.timestamp);

    if ctx.verbose {
        let mut prev_timestamp: Option<DateTime<Utc>> = None;
        for event in ctx.events {
            print_event(event, ctx.verbose, prev_timestamp);
            prev_timestamp = Some(event.timestamp);
        }
    } else {
        print_table(ctx.events);
    }

    let summary_ctx = SummaryContext {
        events: ctx.events,
        first: first_timestamp,
        last: last_timestamp,
        task_id: ctx.task_id,
        ai_summary: ctx.ai_summary,
        mcp_summary: ctx.mcp_summary,
        step_summary: ctx.step_summary,
    };
    print_summary(&summary_ctx);
}

fn build_trace_output(
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

fn filter_log_events(log_events: Vec<TraceEvent>, verbose: bool) -> Vec<TraceEvent> {
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
