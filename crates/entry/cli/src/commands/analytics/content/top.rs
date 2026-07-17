//! `analytics content top` command listing top content by views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{TopContentOutput, TopContentRow};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_to_csv, format_date_range, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct TopArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time")]
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

pub(super) async fn execute_with_pool(
    args: TopArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TopArgs,
    repo: &ContentAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_top_content(start, end, args.limit).await?;

    let content: Vec<TopContentRow> = rows
        .into_iter()
        .map(|row| TopContentRow {
            content: row.content_id.to_string(),
            slug: row.slug.unwrap_or_else(String::new),
            title: row.title.unwrap_or_else(String::new),
            source: row.source_id.map_or_else(String::new, |s| s.to_string()),
            views: row.total_views,
            unique_visitors: row.unique_visitors,
            avg_time_seconds: row.avg_time_on_page_seconds.map_or(0, |v| v as i64),
            trend: row.trend_direction.unwrap_or_else(|| "stable".to_owned()),
        })
        .collect();

    let output = TopContentOutput {
        period: format_date_range(start, end),
        content,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.content, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::table_of(
            vec![
                "slug",
                "title",
                "source",
                "views",
                "unique_visitors",
                "avg_time_seconds",
                "trend",
            ],
            &output.content,
        )
        .with_skip_render());
    }

    if output.content.is_empty() {
        CliService::warning("No content found");
        return Ok(CommandOutput::table_of(
            vec![
                "slug",
                "title",
                "source",
                "views",
                "unique_visitors",
                "avg_time_seconds",
                "trend",
            ],
            &output.content,
        )
        .with_skip_render());
    }

    Ok(CommandOutput::table_of(
        vec![
            "slug",
            "title",
            "source",
            "views",
            "unique_visitors",
            "avg_time_seconds",
            "trend",
        ],
        &output.content,
    )
    .with_title("Top Content"))
}
