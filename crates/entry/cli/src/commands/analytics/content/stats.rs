use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_analytics::ContentAnalyticsRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ContentStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ContentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ContentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ContentAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let row = repo.get_stats(start, end).await?;

    let output = ContentStatsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        total_views: row.total_views,
        unique_visitors: row.unique_visitors,
        avg_time_on_page_seconds: row.avg_time_on_page_seconds.map_or(0, |v| v as i64),
        avg_scroll_depth: row.avg_scroll_depth.unwrap_or(0.0),
        total_clicks: row.total_clicks,
    };

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Content Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

fn render_stats(output: &ContentStatsOutput) {
    CliService::section(&format!("Content Statistics ({})", output.period));

    CliService::key_value("Total Views", &format_number(output.total_views));
    CliService::key_value("Unique Visitors", &format_number(output.unique_visitors));
    CliService::key_value(
        "Avg Time on Page",
        &format!("{}s", output.avg_time_on_page_seconds),
    );
    CliService::key_value("Avg Scroll Depth", &format_percent(output.avg_scroll_depth));
    CliService::key_value("Total Clicks", &format_number(output.total_clicks));
}
