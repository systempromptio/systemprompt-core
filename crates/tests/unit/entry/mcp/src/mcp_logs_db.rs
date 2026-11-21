//! DB-backed tests for the pool-seamed `plugins mcp logs` database mode,
//! driving `execute_db_mode_with_pool` against a fixture pool with seeded
//! log rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_database::DbPool;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_mcp_entry::cli::{logs as mcp_logs, logs_db as mcp_logs_db};
use systemprompt_test_fixtures::{test_db_pool, unique_user_id};
use uuid::Uuid;

fn services() -> systemprompt_manifest::ServicesConfig {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .boot
        .load_services_config()
        .expect("services config")
}

async fn seed_log(pool: &DbPool, module: &str, message: &str) {
    let actor = LogActor::new(
        unique_user_id("mcplogsdb"),
        SessionId::generate(),
        TraceId::generate(),
    );
    let entry = LogEntry::new(LogLevel::Error, module, message, actor);
    LoggingRepository::new(pool).log(entry).await.unwrap();
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&serde_json::to_value(out.artifact()).unwrap())
        .unwrap()
        .contains(needle)
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
async fn mcp_logs_db_returns_matching_entries() {
    let pool = test_db_pool().await;
    let token = format!("covmcp{}", Uuid::new_v4().simple());
    let msg = format!("mcp-log-needle-{}", Uuid::new_v4().simple());
    seed_log(&pool, &format!("mcp.{token}"), &msg).await;

    let out = mcp_logs_db::execute_db_mode_with_pool(&mcp_args(&token, None), &pool, &services())
        .await
        .unwrap();

    assert!(contains(&out, &msg));
    assert!(contains(&out, "database"));
}

#[tokio::test]
async fn mcp_logs_db_level_filter_keeps_error() {
    let pool = test_db_pool().await;
    let token = format!("covmcplvl{}", Uuid::new_v4().simple());
    let msg = format!("mcp-error-needle-{}", Uuid::new_v4().simple());
    seed_log(&pool, &format!("mcp.{token}"), &msg).await;

    let out = mcp_logs_db::execute_db_mode_with_pool(
        &mcp_args(&token, Some(mcp_logs::LogLevel::Error)),
        &pool,
        &services(),
    )
    .await
    .unwrap();

    assert!(contains(&out, &msg));
}
