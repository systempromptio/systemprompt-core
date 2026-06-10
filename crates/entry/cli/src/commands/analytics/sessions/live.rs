use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::CliSessionAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{ActiveSessionRow, LiveSessionsOutput};
use crate::CliConfig;
use crate::commands::analytics::shared::{export_to_csv, resolve_export_path};
use crate::shared::{CommandOutput, render_result};

const LIVE_SESSION_COLUMNS: [&str; 5] = [
    "session_id",
    "user_type",
    "started_at",
    "duration_seconds",
    "request_count",
];

#[derive(Debug, Clone, Args)]
pub struct LiveArgs {
    #[arg(long, default_value = "5", help = "Refresh interval in seconds")]
    pub refresh: u64,

    #[arg(long, help = "Show only once without refresh")]
    pub no_refresh: bool,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum sessions to show"
    )]
    pub limit: i64,

    #[arg(long, help = "Export to CSV (requires --no-refresh)")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: LiveArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = CliSessionAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: LiveArgs,
    repo: &CliSessionAnalyticsRepository,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        let output = fetch_live_sessions(repo, args.limit).await?;
        export_to_csv(&output.sessions, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(
            CommandOutput::table_of(LIVE_SESSION_COLUMNS.to_vec(), &output.sessions)
                .with_skip_render(),
        );
    }

    if args.no_refresh || !config.is_interactive() {
        let output = fetch_live_sessions(repo, args.limit).await?;
        if output.sessions.is_empty() {
            CliService::warning("No active sessions");
            return Ok(
                CommandOutput::table_of(LIVE_SESSION_COLUMNS.to_vec(), &output.sessions)
                    .with_skip_render(),
            );
        }

        return Ok(
            CommandOutput::table_of(LIVE_SESSION_COLUMNS.to_vec(), &output.sessions)
                .with_title("Live Sessions"),
        );
    }

    loop {
        CliService::clear_screen();

        let output = fetch_live_sessions(repo, args.limit).await?;
        render_output(&output, config);

        CliService::info(&format!(
            "\nRefreshing every {}s. Press Ctrl+C to exit.",
            args.refresh
        ));

        tokio::time::sleep(tokio::time::Duration::from_secs(args.refresh)).await;
    }
}

async fn fetch_live_sessions(
    repo: &CliSessionAnalyticsRepository,
    limit: i64,
) -> Result<LiveSessionsOutput> {
    let cutoff = Utc::now() - Duration::minutes(30);

    let rows = repo.get_live_sessions(cutoff, limit).await?;
    let active_count = repo.get_active_count(cutoff).await?;

    let sessions: Vec<ActiveSessionRow> = rows
        .into_iter()
        .map(|row| {
            let current_duration = (Utc::now() - row.started_at).num_seconds();

            ActiveSessionRow {
                session: row.session_id.to_string(),
                user_type: row.user_type.unwrap_or_else(|| "unknown".to_owned()),
                started_at: row.started_at.format("%H:%M:%S").to_string(),
                duration_seconds: row.duration_seconds.map_or(current_duration, i64::from),
                request_count: i64::from(row.request_count.unwrap_or(0)),
                last_activity: row.last_activity_at.format("%H:%M:%S").to_string(),
            }
        })
        .collect();

    Ok(LiveSessionsOutput {
        active_count,
        sessions,
        timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn render_output(output: &LiveSessionsOutput, config: &CliConfig) {
    let result = CommandOutput::table_of(LIVE_SESSION_COLUMNS.to_vec(), &output.sessions)
        .with_title("Live Sessions");
    render_result(&result, config);

    if !config.is_json_output() && output.sessions.is_empty() {
        CliService::warning("No active sessions");
    }
}
