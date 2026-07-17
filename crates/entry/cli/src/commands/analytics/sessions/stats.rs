//! `analytics sessions stats` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::CliSessionAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::SessionStatsOutput;
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = CliSessionAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &CliSessionAnalyticsRepository,
) -> Result<CommandOutput> {
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
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::card_value("Session Statistics", &output).with_skip_render());
    }

    Ok(CommandOutput::card_value("Session Statistics", &output))
}
