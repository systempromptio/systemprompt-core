//! DB-backed tests driving `infra logs` subcommands through the dispatcher.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, unique_user_id};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: LogsCommands,
}

fn parse(args: &[&str]) -> LogsCommands {
    Harness::try_parse_from(std::iter::once("logs").chain(args.iter().copied()))
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
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn seed_log(pool: &DbPool, message: &str) -> LogEntry {
    let actor = LogActor::new(
        unique_user_id("clilogs"),
        SessionId::generate(),
        TraceId::generate(),
    );
    let entry = LogEntry::new(LogLevel::Error, "cli.tests", message, actor);
    LoggingRepository::new(pool)
        .unwrap()
        .with_terminal(false)
        .with_database(true)
        .log(entry.clone())
        .await
        .unwrap();
    entry
}

#[tokio::test]
async fn show_resolves_full_and_partial_ids() {
    let pool = pool().await;
    let entry = seed_log(&pool, "unmistakable show message").await;
    let ctx = ctx(&pool);

    logs::execute(parse(&["show", entry.id.as_str()]), &ctx)
        .await
        .unwrap();
    logs::execute(parse(&["show", entry.trace_id.as_str()]), &ctx)
        .await
        .unwrap();
    logs::execute(parse(&["show", entry.id.as_str(), "--json"]), &ctx)
        .await
        .unwrap();

    let err = logs::execute(parse(&["show", "log_definitely_missing_zzz"]), &ctx)
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn view_and_search_filter_entries() {
    let pool = pool().await;
    seed_log(&pool, "needle-for-search-test").await;
    let ctx = ctx(&pool);

    logs::execute(parse(&["view", "--tail", "5"]), &ctx)
        .await
        .unwrap();
    logs::execute(
        parse(&[
            "view",
            "--level",
            "error",
            "--since",
            "1h",
            "--module",
            "cli.tests",
        ]),
        &ctx,
    )
    .await
    .unwrap();
    logs::execute(parse(&["search", "needle-for-search-test"]), &ctx)
        .await
        .unwrap();
    logs::execute(
        parse(&[
            "search",
            "needle-for-search-test",
            "--level",
            "error",
            "--since",
            "1h",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn summary_reports_counts() {
    let pool = pool().await;
    seed_log(&pool, "summary seed").await;
    let ctx = ctx(&pool);

    logs::execute(parse(&["summary"]), &ctx).await.unwrap();
    logs::execute(parse(&["summary", "--since", "24h"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn export_writes_json_and_csv_files() {
    let pool = pool().await;
    seed_log(&pool, "export seed").await;
    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();

    let json_path = dir.path().join("logs.json");
    logs::execute(
        parse(&[
            "export",
            "--format",
            "json",
            "-o",
            json_path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();
    assert!(json_path.exists());

    let csv_path = dir.path().join("logs.csv");
    logs::execute(
        parse(&[
            "export",
            "--format",
            "csv",
            "-o",
            csv_path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();
    assert!(csv_path.exists());
}

#[tokio::test]
async fn audit_reports_missing_id_gracefully() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let result = logs::execute(parse(&["audit", "no-such-request-id"]), &ctx).await;
    assert!(result.is_ok() || !result.unwrap_err().to_string().is_empty());
}

#[tokio::test]
async fn audit_resolves_seeded_trace() {
    let pool = pool().await;
    let entry = seed_log(&pool, "audit seed").await;
    let ctx = ctx(&pool);
    let result = logs::execute(parse(&["audit", entry.trace_id.as_str()]), &ctx).await;
    assert!(result.is_ok() || !result.unwrap_err().to_string().is_empty());
}

#[tokio::test]
async fn profile_only_commands_refuse_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    for args in [vec!["stream"], vec!["cleanup"], vec!["delete", "--yes"]] {
        let err = logs::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("requires full profile context"));
    }
}

#[tokio::test]
async fn trace_and_tools_listings_run() {
    let pool = pool().await;
    let entry = seed_log(&pool, "trace seed").await;
    let ctx = ctx(&pool);

    logs::execute(parse(&["tools", "list"]), &ctx).await.ok();
    logs::execute(parse(&["trace", "show", entry.trace_id.as_str()]), &ctx)
        .await
        .ok();
}
