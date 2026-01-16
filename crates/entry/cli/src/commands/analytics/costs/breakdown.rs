use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_core_analytics::CostAnalyticsRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

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
    let repo = CostAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: BreakdownArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: BreakdownArgs,
    repo: &CostAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = match args.by {
        BreakdownType::Model => repo.get_breakdown_by_model(start, end, args.limit).await?,
        BreakdownType::Provider => {
            repo.get_breakdown_by_provider(start, end, args.limit)
                .await?
        },
        BreakdownType::Agent => repo.get_breakdown_by_agent(start, end, args.limit).await?,
    };

    let total_cost: i64 = rows.iter().map(|r| r.cost).sum();

    let items: Vec<CostBreakdownItem> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total_cost > 0 {
                (row.cost as f64 / total_cost as f64) * 100.0
            } else {
                0.0
            };

            CostBreakdownItem {
                name: row.name,
                cost_cents: row.cost,
                request_count: row.requests,
                tokens: row.tokens,
                percentage,
            }
        })
        .collect();

    let output = CostBreakdownOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        breakdown_by: format!("{:?}", args.by).to_lowercase(),
        items,
        total_cost_cents: total_cost,
    };

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
