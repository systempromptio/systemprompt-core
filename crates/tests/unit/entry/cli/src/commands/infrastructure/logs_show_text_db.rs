//! `infra logs show` against a text-output config, driving the human-readable
//! renderers (metadata block, per-level trace lines) that the JSON-output
//! suites never reach, plus the id-resolution ladder: full id, trace id,
//! partial id, then not-found.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId};
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

fn ctx(pool: &DbPool, json: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_database(
        cli,
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn store(pool: &DbPool, entry: LogEntry) -> LogEntry {
    LoggingRepository::new(pool)
        .unwrap()
        .with_terminal(false)
        .with_database(true)
        .log(entry.clone())
        .await
        .unwrap();
    entry
}

fn actor(trace: &TraceId) -> LogActor {
    LogActor::new(
        unique_user_id("clishow"),
        SessionId::generate(),
        trace.clone(),
    )
}

#[tokio::test]
async fn a_single_log_renders_its_object_metadata_and_identifier_block_as_text() {
    let pool = pool().await;
    let trace = TraceId::generate();
    let entry = store(
        &pool,
        LogEntry::new(
            LogLevel::Error,
            "cli.tests.show",
            "structured metadata entry",
            actor(&trace),
        )
        .with_metadata(serde_json::json!({"attempt": 2, "reason": "upstream timeout"}))
        .with_task_id(TaskId::generate())
        .with_context_id(ContextId::generate()),
    )
    .await;

    logs::execute(parse(&["show", entry.id.as_str()]), &ctx(&pool, false))
        .await
        .expect("a stored log must render in text mode");

    // The same id under a JSON config must still resolve — the renderer is the
    // only thing that differs.
    logs::execute(parse(&["show", entry.id.as_str()]), &ctx(&pool, true))
        .await
        .expect("a stored log must render in json mode");
}

#[tokio::test]
async fn a_scalar_metadata_blob_renders_without_being_treated_as_a_field_map() {
    let pool = pool().await;
    let trace = TraceId::generate();
    let entry = store(
        &pool,
        LogEntry::new(
            LogLevel::Warn,
            "cli.tests.show",
            "scalar metadata entry",
            actor(&trace),
        )
        .with_metadata(serde_json::json!("a bare string, not an object")),
    )
    .await;

    logs::execute(parse(&["show", entry.id.as_str()]), &ctx(&pool, false))
        .await
        .expect("a non-object metadata blob must not break the renderer");
}

#[tokio::test]
async fn a_trace_id_renders_every_level_through_its_own_text_sink() {
    let pool = pool().await;
    let trace = TraceId::generate();

    for (level, message) in [
        (LogLevel::Error, "failed hard"),
        (LogLevel::Warn, "degraded"),
        (LogLevel::Info, "proceeding"),
        (LogLevel::Debug, "detail"),
    ] {
        store(
            &pool,
            LogEntry::new(level, "cli.tests.show", message, actor(&trace)),
        )
        .await;
    }

    logs::execute(parse(&["show", trace.as_str()]), &ctx(&pool, false))
        .await
        .expect("a trace id must resolve to its whole log set in text mode");

    logs::execute(parse(&["show", trace.as_str()]), &ctx(&pool, true))
        .await
        .expect("a trace id must resolve to its whole log set in json mode");
}

#[tokio::test]
async fn a_partial_id_resolves_only_after_the_full_id_and_trace_lookups_miss() {
    let pool = pool().await;
    let trace = TraceId::generate();
    let entry = store(
        &pool,
        LogEntry::new(
            LogLevel::Info,
            "cli.tests.show",
            "partial id entry",
            actor(&trace),
        ),
    )
    .await;

    let full = entry.id.as_str();
    assert!(
        full.len() > 8,
        "the id must be long enough to truncate, got {full}"
    );
    let prefix = &full[..8];

    logs::execute(parse(&["show", prefix]), &ctx(&pool, false))
        .await
        .expect("a unique id prefix must resolve to the entry");
}

#[tokio::test]
async fn an_id_matching_nothing_is_an_error_that_points_at_logs_view() {
    let pool = pool().await;

    let err = logs::execute(
        parse(&["show", "zzzz-no-such-log-id-anywhere-zzzz"]),
        &ctx(&pool, false),
    )
    .await
    .expect_err("an unmatched id must not render an empty entry");

    let message = err.to_string();
    assert!(
        message.contains("zzzz-no-such-log-id-anywhere-zzzz"),
        "the error must echo the id that was asked for, got {message}"
    );
    assert!(
        message.contains("logs view"),
        "the error must point the operator at the command that lists ids, got {message}"
    );
}
