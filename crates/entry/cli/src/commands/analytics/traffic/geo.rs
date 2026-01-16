use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_analytics::TrafficAnalyticsRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{GeoOutput, GeoRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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

pub async fn execute(args: GeoArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: GeoArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: GeoArgs,
    repo: &TrafficAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_to_csv(&output.countries, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "country".to_string(),
                "session_count".to_string(),
                "percentage".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Geographic Distribution")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_geo(&output);
    }

    Ok(())
}

fn render_geo(output: &GeoOutput) {
    CliService::section(&format!("Geographic Distribution ({})", output.period));
    CliService::key_value("Total Sessions", &format_number(output.total_sessions));

    for geo in &output.countries {
        CliService::key_value(
            &geo.country,
            &format!(
                "{} ({})",
                format_number(geo.session_count),
                format_percent(geo.percentage)
            ),
        );
    }
}
