use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{CostBreakdownItem, CostBreakdownOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_number, format_percent, format_tokens, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BreakdownType {
    Model,
    Agent,
    Provider,
}

#[derive(Debug, Args)]
pub struct BreakdownArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value = "model",
        help = "Breakdown by (model, agent, provider)"
    )]
    pub by: BreakdownType,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum items")]
    pub limit: i64,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: BreakdownArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_breakdown(&pool, start, end, &args.by, args.limit).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.items, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.items.is_empty() {
        CliService::warning("No data found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "name".to_string(),
                "cost_cents".to_string(),
                "request_count".to_string(),
                "tokens".to_string(),
                "percentage".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Cost Breakdown")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_breakdown(&output);
    }

    Ok(())
}

async fn fetch_breakdown(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    by: &BreakdownType,
    limit: i64,
) -> Result<CostBreakdownOutput> {
    let group_field = match by {
        BreakdownType::Model => "model",
        BreakdownType::Agent => {
            "COALESCE((SELECT agent_name FROM agent_tasks at WHERE at.task_id = \
             ai_requests.task_id LIMIT 1), 'unknown')"
        },
        BreakdownType::Provider => "provider",
    };

    let query = format!(
        r"
        SELECT
            {} as name,
            COALESCE(SUM(cost_cents), 0) as cost,
            COUNT(*) as requests,
            COALESCE(SUM(tokens_used), 0) as tokens
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        GROUP BY {}
        ORDER BY SUM(cost_cents) DESC NULLS LAST
        LIMIT $3
        ",
        group_field, group_field
    );

    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(&query)
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(pool.as_ref())
        .await?;

    let total_cost: i64 = rows.iter().map(|(_, c, _, _)| c).sum();

    let items: Vec<CostBreakdownItem> = rows
        .into_iter()
        .map(|(name, cost, requests, tokens)| {
            let percentage = if total_cost > 0 {
                (cost as f64 / total_cost as f64) * 100.0
            } else {
                0.0
            };

            CostBreakdownItem {
                name,
                cost_cents: cost,
                request_count: requests,
                tokens,
                percentage,
            }
        })
        .collect();

    Ok(CostBreakdownOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        breakdown_by: format!("{:?}", by).to_lowercase(),
        items,
        total_cost_cents: total_cost,
    })
}

fn render_breakdown(output: &CostBreakdownOutput) {
    CliService::section(&format!(
        "Cost Breakdown by {} ({})",
        output.breakdown_by, output.period
    ));
    CliService::key_value("Total", &format_cost(output.total_cost_cents));

    for item in &output.items {
        CliService::subsection(&item.name);
        CliService::key_value(
            "Cost",
            &format!(
                "{} ({})",
                format_cost(item.cost_cents),
                format_percent(item.percentage)
            ),
        );
        CliService::key_value("Requests", &format_number(item.request_count));
        CliService::key_value("Tokens", &format_tokens(item.tokens));
    }
}
