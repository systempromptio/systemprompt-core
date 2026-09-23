//! DB-backed tests for `analytics agents list` sorting/export and
//! `analytics sessions live` against seeded rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_cli::analytics::{self, AnalyticsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
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

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn seed_agent_tasks(pool: &DbPool) -> String {
    let user_id = unique_user_id("clianalyticslists");
    let session_id = SessionId::generate();
    let email = format!("{}@clianalyticslists.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();
    let context_id = ContextRepository::new(pool)
        .unwrap()
        .create_context(
            &user_id,
            Some(&session_id),
            "cli-analytics-lists",
            ContextKind::User,
        )
        .await
        .unwrap()
        .to_string();

    let agent = format!("covagent_{}", Uuid::new_v4().simple());
    for status in [
        "TASK_STATE_COMPLETED",
        "TASK_STATE_COMPLETED",
        "TASK_STATE_FAILED",
    ] {
        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, status, agent_name, started_at, \
             completed_at, user_id) VALUES ($1, $2, $3, $4, NOW() - INTERVAL '5 minutes', NOW(), \
             $5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&context_id)
        .bind(status)
        .bind(&agent)
        .bind(user_id.as_str())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }
    agent
}

#[tokio::test]
async fn agents_list_renders_seeded_rows_across_sort_orders() {
    let pool = pool().await;
    let _agent = seed_agent_tasks(&pool).await;
    let ctx = ctx(&pool);

    for sort in ["task-count", "success-rate", "cost", "last-active"] {
        analytics::execute(parse(&["agents", "list", "--sort-by", sort]), &ctx)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn agents_list_exports_csv() {
    let pool = pool().await;
    seed_agent_tasks(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agents.csv");
    analytics::execute(
        parse(&["agents", "list", "--export", path.to_str().unwrap()]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(csv.contains("agent_name"), "{csv}");
}

#[tokio::test]
async fn sessions_live_no_refresh_lists_active_sessions() {
    let pool = pool().await;
    seed_agent_tasks(&pool).await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["sessions", "live", "--no-refresh"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn sessions_live_exports_csv() {
    let pool = pool().await;
    seed_agent_tasks(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.csv");
    analytics::execute(
        parse(&["sessions", "live", "--export", path.to_str().unwrap()]),
        &ctx,
    )
    .await
    .unwrap();

    assert!(path.exists());
}

#[tokio::test]
async fn agents_trends_renders_with_seeded_tasks() {
    let pool = pool().await;
    seed_agent_tasks(&pool).await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["agents", "trends"]), &ctx)
        .await
        .unwrap();
}
