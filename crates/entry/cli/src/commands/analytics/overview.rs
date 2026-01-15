use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::shared::{
    format_cost, format_duration_ms, format_number, format_percent, parse_time_range, CsvBuilder,
    MetricCard,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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
    pub active: i64,
    pub total_today: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct CostMetrics {
    pub total_cents: i64,
    pub change_percent: Option<f64>,
}

pub async fn execute(args: OverviewArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_overview_data(&pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_overview_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::dashboard(output)
            .with_title("Analytics Overview")
            .with_hints(RenderingHints::default());
        render_result(&result);
    } else {
        render_overview(&output);
    }

    Ok(())
}

async fn fetch_overview_data(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<OverviewOutput> {
    let period_duration = end - start;
    let prev_start = start - period_duration;

    let conversations = fetch_conversation_metrics(pool, start, end, prev_start).await?;
    let agents = fetch_agent_metrics(pool, start, end).await?;
    let requests = fetch_request_metrics(pool, start, end).await?;
    let tools = fetch_tool_metrics(pool, start, end).await?;
    let sessions = fetch_session_metrics(pool, start, end).await?;
    let costs = fetch_cost_metrics(pool, start, end, prev_start).await?;

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

async fn fetch_conversation_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    prev_start: DateTime<Utc>,
) -> Result<ConversationMetrics> {
    let current: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_contexts WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let previous: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_contexts WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(prev_start)
    .bind(start)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(ConversationMetrics {
        total: current.0,
        change_percent: calculate_change(current.0, previous.0),
    })
}

async fn fetch_agent_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<AgentMetrics> {
    let stats: (i64, i64, i64) = sqlx::query_as(
        r"
        SELECT
            COUNT(DISTINCT agent_name) as active_agents,
            COUNT(*) as total_tasks,
            COUNT(*) FILTER (WHERE status = 'completed') as completed_tasks
        FROM agent_tasks
        WHERE started_at >= $1 AND started_at < $2
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let success_rate = if stats.1 > 0 {
        (stats.2 as f64 / stats.1 as f64) * 100.0
    } else {
        0.0
    };

    Ok(AgentMetrics {
        active_count: stats.0,
        total_tasks: stats.1,
        success_rate,
    })
}

async fn fetch_request_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<RequestMetrics> {
    let stats: (i64, Option<i64>, Option<f64>) = sqlx::query_as(
        r"
        SELECT
            COUNT(*) as total,
            SUM(tokens_used)::bigint as total_tokens,
            AVG(latency_ms)::float8 as avg_latency
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(RequestMetrics {
        total: stats.0,
        total_tokens: stats.1.unwrap_or(0),
        avg_latency_ms: stats.2.map_or(0, |v| v as i64),
    })
}

async fn fetch_tool_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ToolMetrics> {
    let stats: (i64, i64) = sqlx::query_as(
        r"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status = 'success') as successful
        FROM mcp_tool_executions
        WHERE created_at >= $1 AND created_at < $2
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let success_rate = if stats.0 > 0 {
        (stats.1 as f64 / stats.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(ToolMetrics {
        total_executions: stats.0,
        success_rate,
    })
}

async fn fetch_session_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<SessionMetrics> {
    let active: (i64,) = sqlx::query_as(
        r"
        SELECT COUNT(*)
        FROM user_sessions
        WHERE ended_at IS NULL
          AND last_activity_at >= $1
        ",
    )
    .bind(start)
    .fetch_one(pool.as_ref())
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_sessions WHERE started_at >= $1 AND started_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(SessionMetrics {
        active: active.0,
        total_today: total.0,
    })
}

async fn fetch_cost_metrics(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    prev_start: DateTime<Utc>,
) -> Result<CostMetrics> {
    let current: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(cost_cents) FROM ai_requests WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let previous: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(cost_cents) FROM ai_requests WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(prev_start)
    .bind(start)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(CostMetrics {
        total_cents: current.0.unwrap_or(0),
        change_percent: calculate_change(current.0.unwrap_or(0), previous.0.unwrap_or(0)),
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

fn format_change_percent(change: Option<f64>) -> String {
    change.map_or_else(String::new, |c| {
        let sign = if c >= 0.0 { "+" } else { "" };
        format!("{}{:.1}%", sign, c)
    })
}

fn render_overview(output: &OverviewOutput) {
    CliService::section(&format!("Analytics Overview ({})", output.period));

    let cards = vec![
        MetricCard::new("Conversations", format_number(output.conversations.total))
            .with_change(format_change_percent(output.conversations.change_percent)),
        MetricCard::new("Active Agents", format_number(output.agents.active_count)).with_secondary(
            format!("{} tasks", format_number(output.agents.total_tasks)),
        ),
        MetricCard::new("AI Requests", format_number(output.requests.total)).with_secondary(
            format!(
                "avg {}ms",
                format_duration_ms(output.requests.avg_latency_ms)
            ),
        ),
        MetricCard::new(
            "Tool Executions",
            format_number(output.tools.total_executions),
        )
        .with_secondary(format!(
            "{} success",
            format_percent(output.tools.success_rate)
        )),
        MetricCard::new("Active Sessions", format_number(output.sessions.active)).with_secondary(
            format!("{} total", format_number(output.sessions.total_today)),
        ),
        MetricCard::new("Total Cost", format_cost(output.costs.total_cents))
            .with_change(format_change_percent(output.costs.change_percent)),
    ];

    for card in cards {
        let change_str = card.change.as_deref().unwrap_or("");
        let secondary_str = card.secondary.as_deref().unwrap_or("");
        CliService::key_value(
            &card.label,
            format!("{} {} {}", card.value, change_str, secondary_str).trim(),
        );
    }
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
        "sessions_active",
        "sessions_total",
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
