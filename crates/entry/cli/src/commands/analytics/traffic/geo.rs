use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{GeoOutput, GeoRow};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct GeoArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum countries")]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: GeoArgs, _config: &CliConfig) -> Result<CommandResult<GeoOutput>> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: GeoArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<GeoOutput>> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: GeoArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandResult<GeoOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_geo_breakdown(start, end, args.limit).await?;

    let total: i64 = rows.iter().map(|r| r.count).sum();

    let countries: Vec<GeoRow> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total > 0 {
                (row.count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            GeoRow {
                country: row.country.unwrap_or_else(|| "Unknown".to_string()),
                session_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = GeoOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        countries,
        total_sessions: total,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.countries, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "country".to_string(),
            "session_count".to_string(),
            "percentage".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Geographic Distribution")
        .with_hints(hints))
}
