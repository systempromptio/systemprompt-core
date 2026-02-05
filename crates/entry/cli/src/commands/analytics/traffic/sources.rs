use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{TrafficSourceRow, TrafficSourcesOutput};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SourcesArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum sources")]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,

    #[arg(
        long,
        help = "Include all sessions (ghost sessions, suspected bots that evaded detection)"
    )]
    pub include_all: bool,
}

pub async fn execute(
    args: SourcesArgs,
    _config: &CliConfig,
) -> Result<CommandResult<TrafficSourcesOutput>> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: SourcesArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<TrafficSourcesOutput>> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: SourcesArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandResult<TrafficSourcesOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let engaged_only = !args.include_all;

    let rows = repo
        .get_sources(start, end, args.limit, engaged_only)
        .await?;

    let total: i64 = rows.iter().map(|r| r.count).sum();

    let sources: Vec<TrafficSourceRow> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total > 0 {
                (row.count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            TrafficSourceRow {
                source: row.source.unwrap_or_else(|| "direct".to_string()),
                session_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = TrafficSourcesOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        sources,
        total_sessions: total,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.sources, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "source".to_string(),
            "session_count".to_string(),
            "percentage".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Traffic Sources")
        .with_hints(hints))
}
