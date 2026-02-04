use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{CostBreakdownItem, CostBreakdownOutput};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
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

pub async fn execute(
    args: BreakdownArgs,
    _config: &CliConfig,
) -> Result<CommandResult<CostBreakdownOutput>> {
    let ctx = AppContext::new().await?;
    let repo = CostAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: BreakdownArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<CostBreakdownOutput>> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: BreakdownArgs,
    repo: &CostAnalyticsRepository,
) -> Result<CommandResult<CostBreakdownOutput>> {
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
                cost_microdollars: row.cost,
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
        total_cost_microdollars: total_cost,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.items, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.items.is_empty() {
        CliService::warning("No data found in the specified time range");
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "name".to_string(),
            "cost_microdollars".to_string(),
            "request_count".to_string(),
            "tokens".to_string(),
            "percentage".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Cost Breakdown")
        .with_hints(hints))
}
