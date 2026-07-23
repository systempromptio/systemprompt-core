//! `analytics traffic navigation` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::{NavigationQuery, TrafficAnalyticsRepository};
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{NavigationOutput, NavigationRow};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_to_csv, format_date_range, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct NavigationArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum transitions")]
    pub limit: i64,

    #[arg(
        long,
        help = "Only clicks whose destination starts with this path prefix"
    )]
    pub path_prefix: Option<String>,

    #[arg(long, help = "Include clicks on external links")]
    pub include_external: bool,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: NavigationArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: NavigationArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let internal_only = !args.include_external;

    let rows = repo
        .get_navigation(NavigationQuery {
            start,
            end,
            limit: args.limit,
            path_prefix: args.path_prefix.as_deref(),
            internal_only,
        })
        .await?;

    let total: i64 = rows.iter().map(|r| r.count).sum();

    let transitions: Vec<NavigationRow> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total > 0 {
                (row.count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            NavigationRow {
                from_path: row.from_path.unwrap_or_default(),
                to_path: row.to_path.unwrap_or_default(),
                click_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = NavigationOutput {
        period: format_date_range(start, end),
        transitions,
        total_clicks: total,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.transitions, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::table_of(
            vec!["from_path", "to_path", "click_count", "percentage"],
            &output.transitions,
        )
        .with_skip_render());
    }

    Ok(CommandOutput::table_of(
        vec!["from_path", "to_path", "click_count", "percentage"],
        &output.transitions,
    )
    .with_title("Traffic Navigation"))
}
