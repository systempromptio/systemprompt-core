//! Table-output runs of the analytics dispatcher.
//!
//! The analytics suite standardises on `OutputFormat::Json`, which returns from
//! `render_result` before the terminal renderer. Running the same commands in
//! the default table mode is the only route into the analytics summary
//! printers, and seeded rows keep them off their empty-result arms.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::analytics::{self, AnalyticsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AnalyticsCommands,
}

fn parse(args: &[&str]) -> AnalyticsCommands {
    Harness::try_parse_from(std::iter::once("analytics").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn table_ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn seed(pool: &DbPool) {
    let user_id = unique_user_id("clitableanalytics");
    let email = format!("{}@clitableanalytics.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();

    let session_id = SessionId::generate();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE user_sessions SET landing_page = '/cov-table/home', device_type = 'desktop', \
         browser = 'Firefox', country = 'PT', referrer_source = 'google', is_bot = false, \
         request_count = 5, started_at = NOW() - INTERVAL '2 minutes' WHERE session_id = $1",
    )
    .bind(session_id.as_str())
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();

    let server = format!("covtablesrv_{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         execution_time_ms, input, status, user_id, trace_id, created_at) VALUES ($1, \
         'cov_table_tool', $2, NOW(), 12, '{}', 'success', $3, $4, NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&server)
    .bind(user_id.as_str())
    .bind(Uuid::new_v4().to_string())
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, tokens_used, \
         input_tokens, output_tokens, cost_microdollars, latency_ms, status, actor_kind, \
         actor_id, created_at, completed_at) VALUES ($1, $2, $3, '00000000-0000-0000-0000-00000000c0de', 'covprovider', 'covtablemodel', \
         120, 60, 60, 3000, 210, 'completed', 'user', $3, NOW(), NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(user_id.as_str())
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
}

async fn run_all(args: &[Vec<&str>]) {
    let pool = pool().await;
    seed(&pool).await;
    let ctx = table_ctx(&pool);

    for argv in args {
        analytics::execute(parse(argv), &ctx)
            .await
            .unwrap_or_else(|e| panic!("{argv:?}: {e:#}"));
    }
}

#[tokio::test]
async fn the_overview_renders_its_terminal_summary() {
    run_all(&[vec!["overview"]]).await;
}

#[tokio::test]
async fn traffic_reports_render_their_terminal_summaries() {
    run_all(&[
        vec!["traffic", "sources"],
        vec!["traffic", "geo"],
        vec!["traffic", "devices"],
        vec!["traffic", "bots"],
        vec!["traffic", "pages"],
    ])
    .await;
}

#[tokio::test]
async fn tool_and_request_reports_render_their_terminal_summaries() {
    run_all(&[
        vec!["tools", "stats"],
        vec!["tools", "list"],
        vec!["tools", "trends"],
        vec!["requests", "stats"],
        vec!["requests", "list"],
        vec!["requests", "models"],
        vec!["requests", "trends"],
    ])
    .await;
}

#[tokio::test]
async fn cost_and_session_reports_render_their_terminal_summaries() {
    run_all(&[
        vec!["costs", "summary"],
        vec!["costs", "breakdown"],
        vec!["costs", "trends"],
        vec!["sessions", "stats"],
        vec!["sessions", "trends"],
        vec!["sessions", "live", "--no-refresh"],
    ])
    .await;
}

#[tokio::test]
async fn content_agent_and_conversation_reports_render_their_terminal_summaries() {
    run_all(&[
        vec!["content", "stats"],
        vec!["content", "top"],
        vec!["content", "trends"],
        vec!["agents", "stats"],
        vec!["agents", "list"],
        vec!["agents", "trends"],
        vec!["conversations", "stats"],
        vec!["conversations", "list"],
        vec!["conversations", "trends"],
    ])
    .await;
}
