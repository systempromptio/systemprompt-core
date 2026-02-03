use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::CostSummaryOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_cost, format_number, format_percent, format_tokens,
    parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SummaryArgs {
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

pub async fn execute(args: SummaryArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = CostAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: SummaryArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: SummaryArgs,
    repo: &CostAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let period_duration = end - start;
    let prev_start = start - period_duration;

    let current = repo.get_summary(start, end).await?;
    let previous = repo.get_previous_cost(prev_start, start).await?;

    let total_cost = current.total_cost.unwrap_or(0);
    let prev_cost = previous.cost.unwrap_or(0);
    let change_percent = if prev_cost > 0 {
        Some(((total_cost - prev_cost) as f64 / prev_cost as f64) * 100.0)
    } else {
        None
    };

    let avg_cost = if current.total_requests > 0 {
        total_cost as f64 / current.total_requests as f64
    } else {
        0.0
    };

    let output = CostSummaryOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_cost_microdollars: total_cost,
        total_requests: current.total_requests,
        total_tokens: current.total_tokens.unwrap_or(0),
        avg_cost_per_request_cents: avg_cost,
        change_percent,
    };

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Cost Summary");
        render_result(&result);
    } else {
        render_summary(&output);
    }

    Ok(())
}

fn render_summary(output: &CostSummaryOutput) {
    CliService::section(&format!("Cost Summary ({})", output.period));

    CliService::key_value("Total Cost", &format_cost(output.total_cost_microdollars));
    CliService::key_value("Total Requests", &format_number(output.total_requests));
    CliService::key_value("Total Tokens", &format_tokens(output.total_tokens));
    CliService::key_value(
        "Avg Cost/Request",
        &format_cost(output.avg_cost_per_request_cents as i64),
    );

    if let Some(change) = output.change_percent {
        let sign = if change >= 0.0 { "+" } else { "" };
        CliService::key_value(
            "vs Previous Period",
            &format!("{}{}", sign, format_percent(change)),
        );
    }
}
