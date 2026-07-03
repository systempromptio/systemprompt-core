//! DB-backed tests for the pool-seamed `admin agents logs` and `plugins mcp
//! logs` database modes, driving `execute_db_mode_with_pool` against a fixture
//! pool with seeded log rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::agents::logs as agent_logs;
use systemprompt_cli::admin::agents::logs_db as agent_logs_db;
use systemprompt_cli::plugins::mcp::logs as mcp_logs;
use systemprompt_cli::plugins::mcp::logs_db as mcp_logs_db;
use systemprompt_cli::shared::CommandOutput;
use systemprompt_cli::CliConfig;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, unique_user_id};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

async fn seed_log(pool: &DbPool, module: &str, message: &str) {
    let actor = LogActor::new(
        unique_user_id("clilogsdb"),
        SessionId::generate(),
        TraceId::generate(),
    );
    let entry = LogEntry::new(LogLevel::Error, module, message, actor);
    LoggingRepository::new(pool)
        .unwrap()
        .with_terminal(false)
        .with_database(true)
        .log(entry)
        .await
        .unwrap();
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&serde_json::to_value(out.artifact()).unwrap())
        .unwrap()
        .contains(needle)
}

fn agent_args(agent: &str) -> agent_logs::LogsArgs {
    agent_logs::LogsArgs {
        agent: Some(agent.to_owned()),
        lines: 50,
        follow: false,
        disk: false,
        logs_dir: None,
    }
}

fn mcp_args(server: &str, level: Option<mcp_logs::LogLevel>) -> mcp_logs::LogsArgs {
    mcp_logs::LogsArgs {
        server: Some(server.to_owned()),
        lines: 50,
        follow: false,
        disk: false,
        logs_dir: None,
        level,
    }
}

#[tokio::test]
async fn agent_logs_db_returns_matching_entries() {
    let pool = pool().await;
    let token = format!("covagent{}", Uuid::new_v4().simple());
    let msg = format!("agent-log-needle-{}", Uuid::new_v4().simple());
    seed_log(&pool, &format!("agent.{token}"), &msg).await;

    let out = agent_logs_db::execute_db_mode_with_pool(&agent_args(&token), &pool, &cfg())
        .await
        .unwrap();

    assert!(contains(&out, &msg));
    assert!(contains(&out, "database"));
}

#[tokio::test]
async fn mcp_logs_db_returns_matching_entries() {
    let pool = pool().await;
    let token = format!("covmcp{}", Uuid::new_v4().simple());
    let msg = format!("mcp-log-needle-{}", Uuid::new_v4().simple());
    seed_log(&pool, &format!("mcp.{token}"), &msg).await;

    let out = mcp_logs_db::execute_db_mode_with_pool(&mcp_args(&token, None), &pool, &cfg())
        .await
        .unwrap();

    assert!(contains(&out, &msg));
    assert!(contains(&out, "database"));
}

#[tokio::test]
async fn mcp_logs_db_level_filter_keeps_error() {
    let pool = pool().await;
    let token = format!("covmcplvl{}", Uuid::new_v4().simple());
    let msg = format!("mcp-error-needle-{}", Uuid::new_v4().simple());
    seed_log(&pool, &format!("mcp.{token}"), &msg).await;

    let out = mcp_logs_db::execute_db_mode_with_pool(
        &mcp_args(&token, Some(mcp_logs::LogLevel::Error)),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains(&out, &msg));
}
