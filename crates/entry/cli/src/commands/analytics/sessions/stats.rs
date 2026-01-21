use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::CliSessionAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::SessionStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
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

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = CliSessionAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = CliSessionAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &CliSessionAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let stats = repo.get_stats(start, end).await?;
    let active_count = repo.get_active_session_count(start).await?;

    let conversion_rate = if stats.total_sessions > 0 {
        (stats.conversions as f64 / stats.total_sessions as f64) * 100.0
    } else {
        0.0
    };

    let output = SessionStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_sessions: stats.total_sessions,
        active_sessions: active_count,
        unique_users: stats.unique_users,
        avg_duration_seconds: stats.avg_duration.map_or(0, |v| v as i64),
        avg_requests_per_session: stats.avg_requests.unwrap_or(0.0),
        conversion_rate,
    };

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Session Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

fn render_stats(output: &SessionStatsOutput) {
    CliService::section(&format!("Session Statistics ({})", output.period));

    CliService::key_value("Total Sessions", &format_number(output.total_sessions));
    CliService::key_value("Active Sessions", &format_number(output.active_sessions));
    CliService::key_value("Unique Users", &format_number(output.unique_users));
    CliService::key_value("Avg Duration", &format!("{}s", output.avg_duration_seconds));
    CliService::key_value(
        "Avg Requests/Session",
        &format!("{:.1}", output.avg_requests_per_session),
    );
    CliService::key_value("Conversion Rate", &format_percent(output.conversion_rate));
}
