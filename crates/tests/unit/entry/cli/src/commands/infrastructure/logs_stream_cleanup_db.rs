//! App-context tests driving the profile-only `infra logs` commands (`stream`,
//! `cleanup`) whose loop/mutation bodies require a full runtime context.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_database_url, fixture_db_pool, unique_user_id,
};

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

fn app_ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().unwrap();
    CommandContext::with_app_context(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).unwrap(),
    )
}

async fn seed_log(pool: &DbPool, level: LogLevel, module: &str, message: &str) {
    let actor = LogActor::new(
        unique_user_id("clistream"),
        SessionId::generate(),
        TraceId::generate(),
    );
    let mut entry = LogEntry::new(level, module, message, actor);
    entry.metadata = Some(serde_json::json!({ "probe": message }));
    LoggingRepository::new(pool)
        .unwrap()
        .with_terminal(false)
        .with_database(true)
        .log(entry)
        .await
        .unwrap();
}

#[tokio::test]
async fn stream_bounded_renders_recent_logs() {
    let pool = pool().await;
    let module = format!("cli.stream.{}", uuid::Uuid::new_v4().simple());
    seed_log(&pool, LogLevel::Error, &module, "stream error line").await;
    seed_log(&pool, LogLevel::Info, &module, "stream info line").await;
    let ctx = app_ctx(&pool);

    logs::execute(
        parse(&[
            "stream",
            "--module",
            &module,
            "--level",
            "error",
            "--interval",
            "1",
            "--max-iterations",
            "2",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn stream_clear_and_empty_filter_terminate() {
    let pool = pool().await;
    let ctx = app_ctx(&pool);
    let missing = format!("cli.stream.none.{}", uuid::Uuid::new_v4().simple());

    logs::execute(
        parse(&[
            "stream",
            "--module",
            &missing,
            "--clear",
            "--interval",
            "1",
            "--max-iterations",
            "1",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn stream_rejects_json_output() {
    let pool = pool().await;
    let ctx = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(systemprompt_cli::OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(&pool, &fixture_database_url().unwrap()).unwrap(),
    );
    let err = logs::execute(parse(&["stream", "--max-iterations", "1"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("JSON output"), "{err}");
}

#[tokio::test]
async fn cleanup_dry_run_reports_cutoff() {
    let pool = pool().await;
    let ctx = app_ctx(&pool);
    logs::execute(
        parse(&["cleanup", "--older-than", "3650d", "--dry-run"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn cleanup_keep_last_days_deletes_with_confirmation() {
    let pool = pool().await;
    let ctx = app_ctx(&pool);
    logs::execute(
        parse(&["cleanup", "--keep-last-days", "3650", "--yes"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn cleanup_requires_a_bound() {
    let pool = pool().await;
    let ctx = app_ctx(&pool);
    let err = logs::execute(parse(&["cleanup", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("required"),
        "expected a required-bound error, got: {err}"
    );
}
