//! `analytics content top` command listing top content by views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::ContentAudienceRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContentId;
use systemprompt_logging::CliService;
use systemprompt_traits::usage_reports::content::ContentAudienceReport;

use super::{TopContentOutput, TopContentRow};
use crate::cli::shared::{export_to_csv, format_date_range, parse_time_range};
use systemprompt_extension::cli::{CliConfig, CommandOutput};

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
    pool: &DbPool,
    content: &dyn ContentAudienceReport,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentAudienceRepository::new(pool);
    execute_internal(args, &repo, content).await
}

const COLUMNS: [&str; 7] = [
    "slug",
    "title",
    "source",
    "views",
    "unique_visitors",
    "avg_time_seconds",
    "trend",
];

async fn execute_internal(
    args: TopArgs,
    repo: &ContentAudienceRepository,
    content_report: &dyn ContentAudienceReport,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.top_content_views(start, end, args.limit).await?;
    let ids: Vec<ContentId> = rows.iter().map(|row| row.content_id.clone()).collect();
    let items: HashMap<ContentId, _> = content_report
        .content_items(&ids)
        .await?
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    let content: Vec<TopContentRow> = rows
        .into_iter()
        .map(|row| {
            let item = items.get(&row.content_id);
            TopContentRow {
                content: row.content_id.to_string(),
                slug: item.map_or_else(String::new, |i| i.slug.clone()),
                title: item.map_or_else(String::new, |i| i.title.clone()),
                source: item.map_or_else(String::new, |i| i.source_id.to_string()),
                views: row.views.total_views,
                unique_visitors: row.views.unique_visitors,
                avg_time_seconds: row.views.avg_time_on_page_seconds.map_or(0, |v| v as i64),
                trend: "stable".to_owned(),
            }
        })
        .collect();

    let output = TopContentOutput {
        period: format_date_range(start, end),
        content,
    };

    if let Some(ref path) = args.export {
        export_to_csv(&output.content, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(CommandOutput::table_of(COLUMNS.to_vec(), &output.content).with_skip_render());
    }

    if output.content.is_empty() {
        CliService::warning("No content found");
        return Ok(CommandOutput::table_of(COLUMNS.to_vec(), &output.content).with_skip_render());
    }

    Ok(CommandOutput::table_of(COLUMNS.to_vec(), &output.content).with_title("Top Content"))
}
