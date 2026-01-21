use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{BotRow, BotsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
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

pub async fn execute(args: BotsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: BotsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: BotsArgs,
    repo: &TrafficAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Bot Traffic Analysis");
        render_result(&result);
    } else {
        render_bots(&output);
    }

    Ok(())
}

fn render_bots(output: &BotsOutput) {
    CliService::section(&format!("Bot Traffic Analysis ({})", output.period));

    CliService::key_value("Human Sessions", &format_number(output.human_sessions));
    CliService::key_value("Bot Sessions", &format_number(output.bot_sessions));
    CliService::key_value("Bot Percentage", &format_percent(output.bot_percentage));

    if !output.bot_breakdown.is_empty() {
        CliService::subsection("Bot Breakdown");
        for bot in &output.bot_breakdown {
            CliService::key_value(
                &bot.bot_type,
                &format!(
                    "{} ({})",
                    format_number(bot.request_count),
                    format_percent(bot.percentage)
                ),
            );
        }
    }
}
