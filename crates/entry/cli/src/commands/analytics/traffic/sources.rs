use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{TrafficSourceRow, TrafficSourcesOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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
}

pub async fn execute(args: SourcesArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: SourcesArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: SourcesArgs,
    repo: &TrafficAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_sources(start, end, args.limit).await?;

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
        export_to_csv(&output.sources, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "source".to_string(),
                "session_count".to_string(),
                "percentage".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Traffic Sources")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_sources(&output);
    }

    Ok(())
}

fn render_sources(output: &TrafficSourcesOutput) {
    CliService::section(&format!("Traffic Sources ({})", output.period));
    CliService::key_value("Total Sessions", &format_number(output.total_sessions));

    for source in &output.sources {
        CliService::key_value(
            &source.source,
            &format!(
                "{} ({})",
                format_number(source.session_count),
                format_percent(source.percentage)
            ),
        );
    }
}
