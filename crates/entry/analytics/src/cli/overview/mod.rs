//! Dashboard overview rolling up every analytics domain into one snapshot.
//!
//! [`OverviewArgs`] selects the time range; [`OverviewOutput`] aggregates the
//! per-domain metric cards (conversations, agents, requests, tools, sessions,
//! costs), comparing each against the immediately preceding period of equal
//! length to derive change percentages. Sessions are analytics' own; every
//! other card is read from the report port of the member owning its views
//! ([`OverviewPorts`]), and a card whose member is not admitted is absent and
//! named in `unavailable`. Supports CSV export of the rolled-up row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use systemprompt_analytics::OverviewAnalyticsRepository;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;

mod export;

pub use export::export_overview_csv;

use super::shared::parse_time_range;
use systemprompt_extension::cli::{CliConfig, CommandHost, CommandOutput};
use systemprompt_traits::usage_reports::ReportUnavailable;
use systemprompt_traits::usage_reports::agent::DynAgentActivityReport;
use systemprompt_traits::usage_reports::ai::DynAiUsageReport;
use systemprompt_traits::usage_reports::mcp::DynToolUsageReport;

#[derive(Debug, Args)]
pub struct OverviewArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

/// The overview snapshot. A section whose owning member the composition does
/// not admit is `None`, and `unavailable` names the capability it needs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OverviewOutput {
    pub period: String,
    pub conversations: Option<ConversationMetrics>,
    pub agents: Option<AgentMetrics>,
    pub requests: Option<RequestMetrics>,
    pub tools: Option<ToolMetrics>,
    pub sessions: SessionMetrics,
    pub costs: Option<CostMetrics>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub unavailable: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ConversationMetrics {
    pub total: i64,
    pub change_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AgentMetrics {
    pub active_count: i64,
    pub total_tasks: i64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RequestMetrics {
    pub total: i64,
    pub total_tokens: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ToolMetrics {
    pub total_executions: i64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct SessionMetrics {
    #[serde(rename = "currently_active")]
    pub active: i64,
    #[serde(rename = "created_in_period")]
    pub total_today: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct CostMetrics {
    pub total_cost_microdollars: i64,
    pub change_percent: Option<f64>,
}

/// The report ports the overview reads, each present only when its owning
/// member is admitted.
#[derive(Clone, Default)]
pub struct OverviewPorts {
    pub agents: Option<DynAgentActivityReport>,
    pub ai: Option<DynAiUsageReport>,
    pub tools: Option<DynToolUsageReport>,
}

impl std::fmt::Debug for OverviewPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverviewPorts")
            .field("agents", &self.agents.is_some())
            .field("ai", &self.ai.is_some())
            .field("tools", &self.tools.is_some())
            .finish()
    }
}

impl OverviewPorts {
    pub async fn resolve(host: &dyn CommandHost) -> Result<Self> {
        let resolved = host.resolved().await?;
        Ok(Self {
            agents: resolved.optional::<DynAgentActivityReport>(),
            ai: resolved.optional::<DynAiUsageReport>(),
            tools: resolved.optional::<DynToolUsageReport>(),
        })
    }
}

pub async fn execute_with_pool(
    args: OverviewArgs,
    pool: &DbPool,
    ports: &OverviewPorts,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = OverviewAnalyticsRepository::new(pool);
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_overview_data(&repo, ports, start, end).await?;
    for reason in &output.unavailable {
        CliService::warning(reason);
    }

    if let Some(ref path) = args.export {
        export_overview_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(CommandOutput::card_value("Analytics Overview", &output).with_skip_render());
    }

    Ok(CommandOutput::card_value("Analytics Overview", &output))
}

fn unavailable(report: &'static str, capability: &'static str) -> String {
    ReportUnavailable { report, capability }.to_string()
}

fn rate(part: i64, total: i64) -> f64 {
    if total > 0 {
        (part as f64 / total as f64) * 100.0
    } else {
        0.0
    }
}

async fn fetch_overview_data(
    repo: &OverviewAnalyticsRepository,
    ports: &OverviewPorts,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<OverviewOutput> {
    let prev_start = start - (end - start);
    let mut missing = Vec::new();

    let (conversations, agents) = if let Some(agents) = ports.agents.as_deref() {
        let current = agents.context_count(start, end).await?;
        let previous = agents.context_count(prev_start, start).await?;
        let metrics = agents.agent_metrics(start, end).await?;
        (
            Some(ConversationMetrics {
                total: current,
                change_percent: calculate_change(current, previous),
            }),
            Some(AgentMetrics {
                active_count: metrics.active_agents,
                total_tasks: metrics.total_tasks,
                success_rate: rate(metrics.completed_tasks, metrics.total_tasks),
            }),
        )
    } else {
        missing.push(unavailable(
            "analytics overview conversations and agents",
            "agents",
        ));
        (None, None)
    };

    let (requests, costs) = if let Some(ai) = ports.ai.as_deref() {
        let metrics = ai.request_metrics(start, end).await?;
        let current = ai.overview_cost(start, end).await?.cost.unwrap_or(0);
        let previous = ai.overview_cost(prev_start, start).await?.cost.unwrap_or(0);
        (
            Some(RequestMetrics {
                total: metrics.total,
                total_tokens: metrics.total_tokens.unwrap_or(0),
                avg_latency_ms: metrics.avg_latency.map_or(0, |v| v as i64),
            }),
            Some(CostMetrics {
                total_cost_microdollars: current,
                change_percent: calculate_change(current, previous),
            }),
        )
    } else {
        missing.push(unavailable(
            "analytics overview requests and costs",
            "gateway",
        ));
        (None, None)
    };

    let tools = if let Some(tools) = ports.tools.as_deref() {
        let metrics = tools.overview_tool_metrics(start, end).await?;
        Some(ToolMetrics {
            total_executions: metrics.total,
            success_rate: rate(metrics.successful, metrics.total),
        })
    } else {
        missing.push(unavailable("analytics overview tools", "mcp"));
        None
    };

    let sessions = SessionMetrics {
        active: repo.get_active_session_count(start).await?,
        total_today: repo.get_total_session_count(start, end).await?,
    };

    Ok(OverviewOutput {
        period: format_period(start, end),
        conversations,
        agents,
        requests,
        tools,
        sessions,
        costs,
        unavailable: missing,
    })
}

#[must_use]
pub fn calculate_change(current: i64, previous: i64) -> Option<f64> {
    (previous != 0).then(|| ((current - previous) as f64 / previous as f64) * 100.0)
}

#[must_use]
pub fn format_period(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    )
}
