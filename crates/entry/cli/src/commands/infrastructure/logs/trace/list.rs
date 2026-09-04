//! `infra logs trace list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use systemprompt_logging::{CliService, TraceListFilter, TraceQueryService};
use systemprompt_security::authz::list_trace_ids_with_decision;

use super::{TraceListOutput, TraceListRow};
use crate::CliConfig;
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum number of traces to return"
    )]
    pub limit: i64,

    #[arg(
        long,
        help = "Only show traces since this duration (e.g., '1h', '24h', '7d') or datetime"
    )]
    pub since: Option<String>,

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Filter by status (completed, failed, running)")]
    pub status: Option<String>,

    #[arg(
        long,
        help = "Filter by MCP tool name (shows only traces that used this tool)"
    )]
    pub tool: Option<String>,

    #[arg(
        long,
        help = "Only show traces carrying a governance decision of this verdict: allow, warn, deny, pending"
    )]
    pub decision: Option<String>,

    #[arg(long, help = "Only show traces with MCP tool calls")]
    pub has_mcp: bool,

    #[arg(
        long,
        help = "Include system, untracked, and log-only traces (bridge housekeeping)"
    )]
    pub all: bool,
}

crate::define_pool_command!(ListArgs => (), with_config);

fn build_filter(args: &ListArgs, since_timestamp: Option<DateTime<Utc>>) -> TraceListFilter {
    // Why: the decision filter runs over the returned page, so the page has to
    // be wider than the requested limit or a rare verdict returns nothing on a
    // busy instance while plenty of matching traces exist.
    let sql_limit = if args.decision.is_some() {
        args.limit.saturating_mul(20)
    } else {
        args.limit
    };
    let mut filter = TraceListFilter::new(sql_limit)
        .with_has_mcp(args.has_mcp)
        .with_include_system(args.all);

    if let Some(since) = since_timestamp {
        filter = filter.with_since(since);
    }
    if let Some(agent) = args.agent.clone() {
        filter = filter.with_agent(agent);
    }
    if let Some(status) = args.status.clone() {
        filter = filter.with_status(status);
    }
    if let Some(tool) = args.tool.as_ref().map(|t| format!("%{t}%")) {
        filter = filter.with_tool(tool);
    }
    filter
}

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let filter = build_filter(&args, since_timestamp);

    // Why: filtered after the listing rather than inside it. The trace query
    // already unions four tables to establish which traces exist; joining
    // governance_decisions into it would make every listing pay for a filter
    // almost no listing uses.
    let decision_traces = match args.decision.as_deref() {
        Some(decision) => Some(
            list_trace_ids_with_decision(pool.as_ref(), decision, since_timestamp)
                .await?
                .into_iter()
                .collect::<std::collections::HashSet<_>>(),
        ),
        None => None,
    };

    let service = TraceQueryService::new(Arc::clone(pool));
    let items = service.list_traces(&filter).await?;

    let traces: Vec<TraceListRow> = items
        .into_iter()
        .filter(|r| {
            decision_traces
                .as_ref()
                .is_none_or(|ids| ids.contains(r.trace_id.as_str()))
        })
        .map(|r| {
            let duration_ms = (r.last_timestamp - r.first_timestamp).num_milliseconds();
            TraceListRow {
                trace_id: r.trace_id,
                timestamp: r.first_timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                agent: r.agent,
                status: r.status,
                duration_ms: (duration_ms > 0).then_some(duration_ms),
                ai_requests: r.ai_requests,
                mcp_calls: r.mcp_calls,
            }
        })
        .take(usize::try_from(args.limit.max(0)).unwrap_or(usize::MAX))
        .collect();

    let output = TraceListOutput {
        total: traces.len() as u64,
        traces,
    };

    if output.traces.is_empty() {
        CliService::warning("No traces found");
        return Ok(());
    }

    let result = CommandOutput::table_of(
        vec![
            "trace_id",
            "timestamp",
            "agent",
            "status",
            "duration_ms",
            "ai_requests",
            "mcp_calls",
        ],
        &output.traces,
    )
    .with_title("Recent Traces");

    render_result(&result, config);

    Ok(())
}
