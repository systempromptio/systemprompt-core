//! `analytics traffic geo` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{GeoOutput, GeoRow};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_to_csv, format_date_range, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct GeoArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum countries")]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,

    #[arg(
        long,
        help = "Include all sessions (ghost sessions, suspected bots that evaded detection)"
    )]
    pub include_all: bool,
}

pub(super) async fn execute_with_pool(
    args: GeoArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: GeoArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let engaged_only = !args.include_all;

    let rows = repo
        .get_geo_breakdown(start, end, args.limit, engaged_only)
        .await?;

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
                country: row.country.unwrap_or_else(|| "Unknown".to_owned()),
                session_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = GeoOutput {
        period: format_date_range(start, end),
        countries,
        total_sessions: total,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.countries, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::table_of(
            vec!["country", "session_count", "percentage"],
            &output.countries,
        )
        .with_skip_render());
    }

    Ok(CommandOutput::table_of(
        vec!["country", "session_count", "percentage"],
        &output.countries,
    )
    .with_title("Geographic Distribution"))
}
