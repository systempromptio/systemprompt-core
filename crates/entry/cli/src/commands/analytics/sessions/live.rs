use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{ActiveSessionRow, LiveSessionsOutput};
use crate::commands::analytics::shared::format_number;
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
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
}

pub async fn execute(args: LiveArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    if args.no_refresh || !config.is_interactive() {
        let output = fetch_live_sessions(&pool, args.limit).await?;
        render_output(&output, config);
        return Ok(());
    }

    loop {
        print!("\x1B[2J\x1B[1;1H");

        let output = fetch_live_sessions(&pool, args.limit).await?;
        render_output(&output, config);

        CliService::info(&format!(
            "\nRefreshing every {}s. Press Ctrl+C to exit.",
            args.refresh
        ));

        tokio::time::sleep(tokio::time::Duration::from_secs(args.refresh)).await;
    }
}

async fn fetch_live_sessions(
    pool: &std::sync::Arc<sqlx::PgPool>,
    limit: i64,
) -> Result<LiveSessionsOutput> {
    let cutoff = Utc::now() - Duration::minutes(30);

    let rows: Vec<(
        String,
        String,
        chrono::DateTime<Utc>,
        Option<i64>,
        Option<i64>,
        chrono::DateTime<Utc>,
    )> = sqlx::query_as(
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
    )
    .bind(cutoff)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await?;

    let sessions: Vec<ActiveSessionRow> = rows
        .into_iter()
        .map(
            |(session_id, user_type, started_at, duration, requests, last_activity)| {
                let current_duration = (Utc::now() - started_at).num_seconds();

                ActiveSessionRow {
                    session_id,
                    user_type,
                    started_at: started_at.format("%H:%M:%S").to_string(),
                    duration_seconds: duration.unwrap_or(current_duration),
                    request_count: requests.unwrap_or(0),
                    last_activity: last_activity.format("%H:%M:%S").to_string(),
                }
            },
        )
        .collect();

    let active_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_sessions WHERE ended_at IS NULL AND last_activity_at >= $1",
    )
    .bind(cutoff)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(LiveSessionsOutput {
        active_count: active_count.0,
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
