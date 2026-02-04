use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use systemprompt_analytics::OverviewAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::shared::{parse_time_range, resolve_export_path, CsvBuilder};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct OverviewArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OverviewOutput {
    pub period: String,
    pub conversations: ConversationMetrics,
    pub agents: AgentMetrics,
    pub requests: RequestMetrics,
    pub tools: ToolMetrics,
    pub sessions: SessionMetrics,
    pub costs: CostMetrics,
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
    pub total_cents: i64,
    pub change_percent: Option<f64>,
}

pub async fn execute(
    args: OverviewArgs,
    _config: &CliConfig,
) -> Result<CommandResult<OverviewOutput>> {
    let ctx = AppContext::new().await?;
    let repo = OverviewAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: OverviewArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<OverviewOutput>> {
    let repo = OverviewAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: OverviewArgs,
    repo: &OverviewAnalyticsRepository,
) -> Result<CommandResult<OverviewOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_overview_data(repo, start, end).await?;

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_overview_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::dashboard(output).with_skip_render());
    }

    Ok(CommandResult::dashboard(output)
        .with_title("Analytics Overview")
        .with_hints(RenderingHints::default()))
}

async fn fetch_overview_data(
    repo: &OverviewAnalyticsRepository,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<OverviewOutput> {
    let period_duration = end - start;
    let prev_start = start - period_duration;

    let current_conversations = repo.get_conversation_count(start, end).await?;
    let prev_conversations = repo.get_conversation_count(prev_start, start).await?;

    let conversations = ConversationMetrics {
        total: current_conversations,
        change_percent: calculate_change(current_conversations, prev_conversations),
    };

    let agent_metrics = repo.get_agent_metrics(start, end).await?;
    let success_rate = if agent_metrics.total_tasks > 0 {
        (agent_metrics.completed_tasks as f64 / agent_metrics.total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let agents = AgentMetrics {
        active_count: agent_metrics.active_agents,
        total_tasks: agent_metrics.total_tasks,
        success_rate,
    };

    let request_metrics = repo.get_request_metrics(start, end).await?;
    let requests = RequestMetrics {
        total: request_metrics.total,
        total_tokens: request_metrics.total_tokens.unwrap_or(0),
        avg_latency_ms: request_metrics.avg_latency.map_or(0, |v| v as i64),
    };

    let tool_metrics = repo.get_tool_metrics(start, end).await?;
    let tool_success_rate = if tool_metrics.total > 0 {
        (tool_metrics.successful as f64 / tool_metrics.total as f64) * 100.0
    } else {
        0.0
    };

    let tools = ToolMetrics {
        total_executions: tool_metrics.total,
        success_rate: tool_success_rate,
    };

    let active_sessions = repo.get_active_session_count(start).await?;
    let total_sessions = repo.get_total_session_count(start, end).await?;

    let sessions = SessionMetrics {
        active: active_sessions,
        total_today: total_sessions,
    };

    let current_cost = repo.get_cost(start, end).await?;
    let prev_cost = repo.get_cost(prev_start, start).await?;

    let costs = CostMetrics {
        total_cents: current_cost.cost.unwrap_or(0),
        change_percent: calculate_change(
            current_cost.cost.unwrap_or(0),
            prev_cost.cost.unwrap_or(0),
        ),
    };

    Ok(OverviewOutput {
        period: format_period(start, end),
        conversations,
        agents,
        requests,
        tools,
        sessions,
        costs,
    })
}

fn calculate_change(current: i64, previous: i64) -> Option<f64> {
    (previous != 0).then(|| ((current - previous) as f64 / previous as f64) * 100.0)
}

fn format_period(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    )
}

fn export_overview_csv(output: &OverviewOutput, path: &std::path::Path) -> Result<()> {
    let mut csv = CsvBuilder::new().headers(vec![
        "period",
        "conversations_total",
        "conversations_change_pct",
        "agents_active",
        "agents_tasks",
        "agents_success_rate",
        "requests_total",
        "requests_tokens",
        "requests_avg_latency_ms",
        "tools_executions",
        "tools_success_rate",
        "sessions_currently_active",
        "sessions_created_in_period",
        "costs_cents",
        "costs_change_pct",
    ]);

    csv.add_row(vec![
        output.period.clone(),
        output.conversations.total.to_string(),
        output
            .conversations
            .change_percent
            .map_or(String::new(), |v| format!("{:.2}", v)),
        output.agents.active_count.to_string(),
        output.agents.total_tasks.to_string(),
        format!("{:.2}", output.agents.success_rate),
        output.requests.total.to_string(),
        output.requests.total_tokens.to_string(),
        output.requests.avg_latency_ms.to_string(),
        output.tools.total_executions.to_string(),
        format!("{:.2}", output.tools.success_rate),
        output.sessions.active.to_string(),
        output.sessions.total_today.to_string(),
        output.costs.total_cents.to_string(),
        output
            .costs
            .change_percent
            .map_or(String::new(), |v| format!("{:.2}", v)),
    ]);

    csv.write_to_file(path)
}
