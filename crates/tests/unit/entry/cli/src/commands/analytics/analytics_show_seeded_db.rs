//! DB-backed tests for `analytics tools show` and `analytics agents show`
//! against seeded execution/task rows, including the CSV export paths.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
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

async fn seed_tool_execution(
    pool: &DbPool,
    tool: &str,
    status: &str,
    error: Option<&str>,
    task_id: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         execution_time_ms, input, status, error_message, user_id, task_id, created_at) VALUES \
         ($1, $2, 'stub-server', NOW(), 25, '{}', $3, $4, 'analytics-show-user', $5, NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(tool)
    .bind(status)
    .bind(error)
    .bind(task_id)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
}

async fn seed_context(pool: &DbPool) -> String {
    let user_id = unique_user_id("clianalyticsshow");
    let session_id = SessionId::generate();
    let email = format!("{}@clianalyticsshow.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();
    ContextRepository::new(pool)
        .unwrap()
        .create_context(&user_id, Some(&session_id), "cli-analytics-show")
        .await
        .unwrap()
        .to_string()
}

async fn seed_agent_task(
    pool: &DbPool,
    context_id: &str,
    task_id: &str,
    agent: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, agent_name, started_at, user_id) \
         VALUES ($1, $2, $3, $4, NOW(), 'analytics-show-user')",
    )
    .bind(task_id)
    .bind(context_id)
    .bind(status)
    .bind(agent)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
}

async fn seed_tool_fixture(pool: &DbPool) -> String {
    let tool = format!("covtool_{}", Uuid::new_v4().simple());
    let context_id = seed_context(pool).await;
    let linked_task = Uuid::new_v4().to_string();
    seed_agent_task(
        pool,
        &context_id,
        &linked_task,
        "cov-linked-agent",
        "TASK_STATE_COMPLETED",
    )
    .await;
    seed_tool_execution(pool, &tool, "success", None, Some(&linked_task)).await;
    seed_tool_execution(pool, &tool, "success", None, None).await;
    seed_tool_execution(pool, &tool, "failed", Some("boom failure"), None).await;
    seed_tool_execution(pool, &tool, "timeout", None, None).await;
    tool
}

async fn seed_agent_fixture(pool: &DbPool) -> String {
    let agent = format!("covagent_{}", Uuid::new_v4().simple());
    let context_id = seed_context(pool).await;
    for status in [
        "TASK_STATE_COMPLETED",
        "TASK_STATE_FAILED",
        "TASK_STATE_WORKING",
    ] {
        seed_agent_task(
            pool,
            &context_id,
            &Uuid::new_v4().to_string(),
            &agent,
            status,
        )
        .await;
    }
    agent
}

#[tokio::test]
async fn tools_show_renders_seeded_activity() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let tool = seed_tool_fixture(&pool).await;

    analytics::execute(parse(&["tools", "show", &tool]), &ctx)
        .await
        .unwrap();
    analytics::execute(parse(&["tools", "show", &tool, "--since", "1h"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn tools_show_exports_csv() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let tool = seed_tool_fixture(&pool).await;
    let dir = tempfile::tempdir().unwrap();
    let export = dir.path().join("tools.csv");

    analytics::execute(
        parse(&["tools", "show", &tool, "--export", export.to_str().unwrap()]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&export).unwrap();
    assert!(csv.contains("tool_name"));
    assert!(csv.contains(&tool));
}

#[tokio::test]
async fn tools_show_unknown_tool_errors() {
    let pool = pool().await;
    let err = analytics::execute(parse(&["tools", "show", "no-such-cov-tool"]), &ctx(&pool))
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("not found"));
}

#[tokio::test]
async fn agents_show_renders_seeded_activity() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let agent = seed_agent_fixture(&pool).await;

    analytics::execute(parse(&["agents", "show", &agent]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn agents_show_exports_csv() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let agent = seed_agent_fixture(&pool).await;
    let dir = tempfile::tempdir().unwrap();
    let export = dir.path().join("agents.csv");

    analytics::execute(
        parse(&[
            "agents",
            "show",
            &agent,
            "--export",
            export.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&export).unwrap();
    assert!(csv.contains("agent_name"));
    assert!(csv.contains(&agent));
}
