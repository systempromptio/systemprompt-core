use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_core_logging::{AiTraceService, CliService, TraceEvent, TraceQueryService};
use systemprompt_runtime::AppContext;

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_execution_steps, print_task_info,
    print_user_input,
};
use super::ai_mcp::print_mcp_executions;
use super::display::{print_event, print_table};
use super::json::print_json;
use super::summary::{print_summary, SummaryContext};
use crate::CliConfig;

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

struct FormattedDisplayContext<'a> {
    events: &'a [TraceEvent],
    trace_id: &'a str,
    task_id: Option<&'a str>,
    verbose: bool,
    ai_summary: &'a systemprompt_core_logging::AiRequestSummary,
    mcp_summary: &'a systemprompt_core_logging::McpExecutionSummary,
    step_summary: &'a systemprompt_core_logging::ExecutionStepSummary,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let _ = config;
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    // First try to resolve as a task ID for detailed AI trace view
    let ai_service = AiTraceService::new(pool.clone());
    if let Ok(task_id) = ai_service.resolve_task_id(&args.id).await {
        return execute_ai_trace(&ai_service, &task_id, &args).await;
    }

    // Otherwise treat as a trace ID for event-based view
    execute_trace_view(&args, &pool).await
}

async fn execute_trace_view(args: &ShowArgs, pool: &std::sync::Arc<sqlx::PgPool>) -> Result<()> {
    let service = TraceQueryService::new(pool.clone());

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

    if events.is_empty() {
        if ai_summary.request_count > 0 || mcp_summary.execution_count > 0 {
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
                let cost_dollars = f64::from(ai_summary.total_cost_cents as i32) / 1_000_000.0;
                CliService::key_value("Cost", &format!("${:.6}", cost_dollars));
            }
            if mcp_summary.execution_count > 0 {
                CliService::key_value("MCP Calls", &mcp_summary.execution_count.to_string());
            }
            CliService::info("Use --verbose to see all log entries, or --ai/--mcp for details");
            return Ok(());
        }
        CliService::warning(&format!("No events found for trace: {}", args.id));
        CliService::info(
            "Tip: The trace may take a moment to populate. Try again in a few seconds.",
        );
        return Ok(());
    }

    if args.json {
        print_json(&events, &args.id, &ai_summary, &mcp_summary, &step_summary);
    } else {
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
    }

    Ok(())
}

async fn execute_ai_trace(service: &AiTraceService, task_id: &str, args: &ShowArgs) -> Result<()> {
    CliService::section(&format!("Trace: {}", task_id));

    let task_info = service.get_task_info(task_id).await?;
    let context_id = task_info.context_id.clone();

    print_task_info(&task_info);

    let user_input = service.get_user_input(task_id).await?;
    print_user_input(user_input.as_ref());

    let show_all = args.all;

    // Show execution steps
    if show_all || args.steps {
        let steps = service.get_execution_steps(task_id).await?;
        print_execution_steps(&steps);
    }

    // Show AI requests
    if show_all || args.ai {
        let ai_requests = service.get_ai_requests(task_id).await?;
        print_ai_requests(&ai_requests);
    }

    // Show MCP executions
    if show_all || args.mcp {
        let mcp_executions = service.get_mcp_executions(task_id, &context_id).await?;
        print_mcp_executions(service, &mcp_executions, task_id, &context_id, args.verbose).await;
    }

    // Show artifacts
    if show_all || args.artifacts {
        let artifacts = service.get_task_artifacts(task_id, &context_id).await?;
        print_artifacts(&artifacts);
    }

    let response = service.get_agent_response(task_id).await?;
    print_agent_response(response.as_ref());

    CliService::info(&"═".repeat(60));

    Ok(())
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
