use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{BotRow, BotsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct BotsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: BotsArgs, _config: &CliConfig) -> Result<CommandResult<BotsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: BotsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<BotsOutput>> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: BotsArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandResult<BotsOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let totals = repo.get_bot_totals(start, end).await?;
    let bot_types = repo.get_bot_breakdown(start, end).await?;

    let total = totals.human + totals.bot;
    let bot_percentage = if total > 0 {
        (totals.bot as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let bot_breakdown: Vec<BotRow> = bot_types
        .into_iter()
        .map(|row| {
            let percentage = if totals.bot > 0 {
                (row.count as f64 / totals.bot as f64) * 100.0
            } else {
                0.0
            };
            BotRow {
                bot_type: row.bot_type.unwrap_or_else(|| "Unknown".to_string()),
                request_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = BotsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        human_sessions: totals.human,
        bot_sessions: totals.bot,
        bot_percentage,
        bot_breakdown,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("Bot Traffic Analysis"))
}
