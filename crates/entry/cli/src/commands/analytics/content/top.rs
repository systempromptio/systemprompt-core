use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{TopContentOutput, TopContentRow};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TopArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum content items"
    )]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: TopArgs,
    _config: &CliConfig,
) -> Result<CommandResult<TopContentOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: TopArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<TopContentOutput>> {
    let repo = ContentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TopArgs,
    repo: &ContentAnalyticsRepository,
) -> Result<CommandResult<TopContentOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_top_content(start, end, args.limit).await?;

    let content: Vec<TopContentRow> = rows
        .into_iter()
        .map(|row| TopContentRow {
            content_id: row.content_id.to_string(),
            views: row.total_views,
            unique_visitors: row.unique_visitors,
            avg_time_seconds: row.avg_time_on_page_seconds.map_or(0, |v| v as i64),
            trend: row.trend_direction.unwrap_or_else(|| "stable".to_string()),
        })
        .collect();

    let output = TopContentOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        content,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.content, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.content.is_empty() {
        CliService::warning("No content found");
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "content_id".to_string(),
            "views".to_string(),
            "unique_visitors".to_string(),
            "avg_time_seconds".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Top Content")
        .with_hints(hints))
}
