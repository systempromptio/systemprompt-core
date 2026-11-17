//! `infra logs trace show`: assemble and render a full execution trace.
//!
//! Defines the [`ShowArgs`] / [`TraceSections`] CLI surface, resolves the id to
//! either an AI task trace or a log-event trace, and merges log, AI, MCP, and
//! step events into a single time-ordered view.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_identifiers::{TaskId, TraceId};
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{AiTraceService, TraceEvent, TraceQueryService, TraceRepository};

use super::ai_trace_display::{execute_ai_trace, filter_log_events};
use super::display::{push_event, push_table};
use super::json::build_json;
use super::output::{TraceSummaries, build_trace_output};
use super::summary::{SummaryContext, line, push_summary};
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Trace ID or Task ID (can be partial)")]
    pub id: String,

    #[arg(long, help = "Show detailed metadata for each event")]
    pub verbose: bool,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[command(flatten)]
    pub sections: TraceSections,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct TraceSections {
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
    task_id: Option<&'a TaskId>,
    verbose: bool,
    ai_summary: &'a systemprompt_runtime::AiRequestSummary,
    mcp_summary: &'a systemprompt_runtime::McpExecutionSummary,
    step_summary: &'a systemprompt_runtime::ExecutionStepSummary,
}

pub(super) async fn execute(args: ShowArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let pool = pool_from_handle(host.db().await?)?.pool();
    execute_with_pool_inner(args, &pool, host.output().is_json_output()).await
}

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
    machine_output: bool,
) -> Result<CommandOutput> {
    let ai_service = AiTraceService::new(TraceRepository::new(Arc::clone(pool)));
    if let Ok(task_id) = ai_service.resolve_task_id(&args.id).await {
        return execute_ai_trace(&ai_service, &task_id, &args, machine_output).await;
    }

    execute_trace_view(&args, pool, machine_output).await
}

async fn execute_trace_view(
    args: &ShowArgs,
    pool: &Arc<sqlx::PgPool>,
    machine_output: bool,
) -> Result<CommandOutput> {
    let service = TraceQueryService::new(TraceRepository::new(Arc::clone(pool)));
    let trace_id = TraceId::try_new(args.id.as_str())?;

    let (
        log_events,
        ai_events,
        mcp_events,
        step_events,
        ai_summary,
        mcp_summary,
        step_summary,
        task_id,
    ) = service.get_all_trace_data(&trace_id).await?;
    let task_id: Option<TaskId> = task_id.map(TaskId::new);

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
    if args.json {
        return Ok(build_json(
            &events,
            &trace_id,
            &ai_summary,
            &mcp_summary,
            &step_summary,
        ));
    }
    if machine_output {
        let output = build_trace_output(
            &trace_id,
            &events,
            &summaries,
            task_id.as_ref(),
            duration_ms,
        );
        return Ok(CommandOutput::card_value("Trace Details", &output));
    }

    let mut lines = Vec::new();
    if events.is_empty() {
        push_empty_trace(&mut lines, &trace_id, &ai_summary, &mcp_summary);
        return Ok(CommandOutput::message(lines));
    }

    let display_ctx = FormattedDisplayContext {
        events: &events,
        task_id: task_id.as_ref(),
        verbose: args.verbose,
        ai_summary: &ai_summary,
        mcp_summary: &mcp_summary,
        step_summary: &step_summary,
    };
    push_formatted(&mut lines, &display_ctx);
    Ok(CommandOutput::message(lines).with_title(format!("Trace Flow: {trace_id}")))
}

fn push_empty_trace(
    out: &mut Vec<NoticeLine>,
    trace_id: &TraceId,
    ai_summary: &systemprompt_runtime::AiRequestSummary,
    mcp_summary: &systemprompt_runtime::McpExecutionSummary,
) {
    if ai_summary.request_count == 0 && mcp_summary.execution_count == 0 {
        line(
            out,
            "warning",
            format!("No events found for trace: {trace_id}"),
        );
        line(
            out,
            "info",
            "Tip: The trace may take a moment to populate. Try again in a few seconds.",
        );
        return;
    }

    line(out, "info", format!("Trace: {trace_id}"));
    line(out, "info", "No log events found, but trace has activity:");
    if ai_summary.request_count > 0 {
        line(
            out,
            "info",
            format!("AI Requests: {}", ai_summary.request_count),
        );
        line(
            out,
            "info",
            format!(
                "Total Tokens: {} in / {} out",
                ai_summary.total_input_tokens, ai_summary.total_output_tokens
            ),
        );
        let cost_dollars = f64::from(ai_summary.total_cost_microdollars as i32) / 1_000_000.0;
        line(out, "info", format!("Cost: ${cost_dollars:.6}"));
    }
    if mcp_summary.execution_count > 0 {
        line(
            out,
            "info",
            format!("MCP Calls: {}", mcp_summary.execution_count),
        );
    }
    line(
        out,
        "info",
        "Use --verbose to see all log entries, or --ai/--mcp for details",
    );
}

fn push_formatted(out: &mut Vec<NoticeLine>, ctx: &FormattedDisplayContext<'_>) {
    let first_timestamp = ctx.events.first().map(|e| e.timestamp);
    let last_timestamp = ctx.events.last().map(|e| e.timestamp);

    if ctx.verbose {
        let mut prev_timestamp: Option<DateTime<Utc>> = None;
        for event in ctx.events {
            push_event(out, event, ctx.verbose, prev_timestamp);
            prev_timestamp = Some(event.timestamp);
        }
    } else {
        push_table(out, ctx.events);
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
    push_summary(out, &summary_ctx);
}
