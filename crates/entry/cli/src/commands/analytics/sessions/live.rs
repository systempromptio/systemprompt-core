use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ActiveSessionRow, LiveSessionsOutput};
use crate::commands::analytics::shared::{export_to_csv, format_number};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

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

pub async fn execute(args: LiveArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: LiveArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    let mut args = args;
    args.no_refresh = true;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: LiveArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    if let Some(ref path) = args.export {
        let output = fetch_live_sessions(pool, args.limit).await?;
        export_to_csv(&output.sessions, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if args.no_refresh || !config.is_interactive() {
        let output = fetch_live_sessions(pool, args.limit).await?;
        render_output(&output, config);
        return Ok(());
    }

    loop {
        CliService::clear_screen();

        let output = fetch_live_sessions(pool, args.limit).await?;
        render_output(&output, config);

        CliService::info(&format!(
            "\nRefreshing every {}s. Press Ctrl+C to exit.",
            args.refresh
        ));

        tokio::time::sleep(tokio::time::Duration::from_secs(args.refresh)).await;
    }
}

async fn fetch_live_sessions(
    pool: &Arc<sqlx::PgPool>,
    limit: i64,
) -> Result<LiveSessionsOutput> {
    let cutoff = Utc::now() - Duration::minutes(30);

    struct SessionRow {
        session_id: String,
        user_type: Option<String>,
        started_at: chrono::DateTime<Utc>,
        duration_seconds: Option<i32>,
        request_count: Option<i32>,
        last_activity_at: chrono::DateTime<Utc>,
    }

    let rows = sqlx::query_as!(
        SessionRow,
        r#"
        SELECT
            session_id,
            COALESCE(user_type, 'unknown') as user_type,
            started_at,
            duration_seconds,
            request_count,
            last_activity_at
        FROM user_sessions
        WHERE ended_at IS NULL
          AND last_activity_at >= $1
        ORDER BY last_activity_at DESC
        LIMIT $2
        "#,
        cutoff,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?;

    let sessions: Vec<ActiveSessionRow> = rows
        .into_iter()
        .map(|row| {
            let current_duration = (Utc::now() - row.started_at).num_seconds();

            ActiveSessionRow {
                session_id: row.session_id,
                user_type: row.user_type.unwrap_or_else(|| "unknown".to_string()),
                started_at: row.started_at.format("%H:%M:%S").to_string(),
                duration_seconds: row.duration_seconds.map_or(current_duration, i64::from),
                request_count: i64::from(row.request_count.unwrap_or(0)),
                last_activity: row.last_activity_at.format("%H:%M:%S").to_string(),
            }
        })
        .collect();

    let active_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM user_sessions WHERE ended_at IS NULL AND last_activity_at >= $1"#,
        cutoff
    )
    .fetch_one(pool.as_ref())
    .await?;

    Ok(LiveSessionsOutput {
        active_count,
        sessions,
        timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn render_output(output: &LiveSessionsOutput, config: &CliConfig) {
    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "session_id".to_string(),
                "user_type".to_string(),
                "started_at".to_string(),
                "duration_seconds".to_string(),
                "request_count".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output.clone())
            .with_title("Live Sessions")
            .with_hints(hints);
        render_result(&result);
        return;
    }

    CliService::section(&format!(
        "Live Sessions ({}) - {}",
        format_number(output.active_count),
        output.timestamp
    ));

    if output.sessions.is_empty() {
        CliService::warning("No active sessions");
        return;
    }

    for session in &output.sessions {
        CliService::info(&format!(
            "{} | {} | {}s | {} requests | last: {}",
            &session.session_id[..8.min(session.session_id.len())],
            session.user_type,
            session.duration_seconds,
            session.request_count,
            session.last_activity
        ));
    }
}
