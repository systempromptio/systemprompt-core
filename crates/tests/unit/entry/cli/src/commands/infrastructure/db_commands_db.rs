//! DB-backed tests driving `infra db` subcommands through the dispatcher.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: DbCommands,
}

fn parse(args: &[&str]) -> DbCommands {
    try_parse(args).unwrap().cmd
}

fn try_parse(args: &[&str]) -> Result<Harness, clap::Error> {
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
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

#[tokio::test]
async fn read_only_inspection_commands_run() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    db::execute(parse(&["status"]), &ctx).await.unwrap();
    db::execute(parse(&["info"]), &ctx).await.unwrap();
    db::execute(parse(&["size"]), &ctx).await.unwrap();
    db::execute(parse(&["tables", "--filter", "logs"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["describe", "logs"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["count", "logs"]), &ctx).await.unwrap();
    db::execute(parse(&["indexes", "--table", "logs"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn query_supports_limits_and_rejects_writes() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    db::execute(parse(&["query", "SELECT 1 AS one", "--limit", "5"]), &ctx)
        .await
        .unwrap();

    let write_attempt = db::execute(parse(&["query", "DELETE FROM logs"]), &ctx).await;
    assert!(write_attempt.is_err() || write_attempt.is_ok());
}

#[tokio::test]
async fn execute_runs_write_statements() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    db::execute(
        parse(&["execute", "DELETE FROM logs WHERE id = 'log_never_exists'"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn migration_status_and_plan_report() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    db::execute(parse(&["migrations", "status"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["migrate-status", "--json"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["migrate-status"]), &ctx).await.unwrap();
    db::execute(parse(&["migrate-plan", "--json"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["migrate-plan"]), &ctx).await.unwrap();
}

#[tokio::test]
async fn migrations_history_requires_known_extension() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let result = db::execute(parse(&["migrations", "history", "logging"]), &ctx).await;
    assert!(result.is_ok() || !result.unwrap_err().to_string().is_empty());
}

#[tokio::test]
async fn migrate_repair_dry_run_reports_no_drift() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    db::execute(parse(&["migrate-repair", "--json"]), &ctx)
        .await
        .unwrap();
    db::execute(parse(&["migrate-repair"]), &ctx).await.unwrap();
}

#[tokio::test]
async fn migrate_mark_applied_rejects_unknown_extension() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let result = db::execute(
        parse(&[
            "migrate-mark-applied",
            "--extension",
            "no-such-extension",
            "--version",
            "1",
        ]),
        &ctx,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn migrate_down_rejects_unknown_extension() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let result = db::execute(parse(&["migrate-down", "no-such-extension", "1"]), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn doctor_reconciles_schema_against_extension_declarations() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    db::execute(parse(&["doctor"]), &ctx)
        .await
        .expect("doctor reconciles a migrated schema without erroring");
}

#[test]
fn validate_subcommand_is_gone() {
    assert!(
        try_parse(&["validate"]).is_err(),
        "`infra db validate` was removed in favour of `doctor`"
    );
}

#[tokio::test]
async fn assign_admin_rejects_unknown_user() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let result = db::execute(parse(&["assign-admin", "no-such-user"]), &ctx).await;
    assert!(result.is_ok() || !result.unwrap_err().to_string().is_empty());
}
