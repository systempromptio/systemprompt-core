use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_core_logging::{CliService, TraceEvent, TraceQueryService};
use systemprompt_runtime::AppContext;

use super::client::send_and_trace;
use super::display::{print_event, print_table};
use super::summary::{print_summary, SummaryContext};
use super::json::print_json;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TraceOptions {
    #[arg(long, help = "Specify agent name")]
    pub agent: Option<String>,

    #[arg(long, short = 'm', help = "Custom message to send")]
    pub message: Option<String>,

    #[arg(long, help = "Show detailed metadata for each event")]
    pub verbose: bool,

    #[arg(long, help = "Output as JSON instead of formatted text")]
    pub json: bool,
}

struct FormattedDisplayContext<'a> {
    events: &'a [TraceEvent],
    trace_id: &'a str,
    task_id: Option<&'a str>,
    options: &'a TraceOptions,
    ai_summary: &'a systemprompt_core_logging::AiRequestSummary,
    mcp_summary: &'a systemprompt_core_logging::McpExecutionSummary,
    step_summary: &'a systemprompt_core_logging::ExecutionStepSummary,
}

fn print_formatted(ctx: &FormattedDisplayContext<'_>) {
    CliService::section(&format!("Trace Flow: {}", ctx.trace_id));

    let first_timestamp = ctx.events.first().map(|e| e.timestamp);
    let last_timestamp = ctx.events.last().map(|e| e.timestamp);

    if ctx.options.verbose {
        let mut prev_timestamp: Option<DateTime<Utc>> = None;
        for event in ctx.events {
            print_event(event, ctx.options.verbose, prev_timestamp);
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

pub async fn execute(trace_id: Option<&str>, options: TraceOptions, config: &CliConfig) -> Result<()> {
    let _ = config; // Will be used when we convert to CommandResult
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let effective_trace_id = if let Some(id) = trace_id {
        id.to_string()
    } else {
        let config = systemprompt_models::Config::get()?;
        let base_url = config.api_external_url.clone();

        send_and_trace(&options, &base_url).await?
    };

    let service = TraceQueryService::new(pool);
    let (
        log_events,
        ai_events,
        mcp_events,
        step_events,
        ai_summary,
        mcp_summary,
        step_summary,
        task_id,
    ) = service.get_all_trace_data(&effective_trace_id).await?;

    let filtered_log_events = filter_log_events(log_events, options.verbose);

    let mut events = filtered_log_events;
    events.extend(ai_events);
    events.extend(mcp_events);
    events.extend(step_events);
    events.sort_by_key(|e| e.timestamp);

    if events.is_empty() {
        CliService::warning(&format!(
            "No events found for trace_id: {effective_trace_id}"
        ));
        CliService::info(
            "Tip: The trace may take a moment to populate. Try again in a few seconds.",
        );
        return Ok(());
    }

    if options.json {
        print_json(
            &events,
            &effective_trace_id,
            &ai_summary,
            &mcp_summary,
            &step_summary,
        );
    } else {
        let display_ctx = FormattedDisplayContext {
            events: &events,
            trace_id: &effective_trace_id,
            task_id: task_id.as_deref(),
            options: &options,
            ai_summary: &ai_summary,
            mcp_summary: &mcp_summary,
            step_summary: &step_summary,
        };
        print_formatted(&display_ctx);
    }

    Ok(())
}
