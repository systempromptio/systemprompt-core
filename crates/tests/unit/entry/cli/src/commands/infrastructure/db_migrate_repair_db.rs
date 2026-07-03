//! DB-backed tests for `infra db migrate-repair` drift detection and the
//! apply path, driven through the standalone dispatcher.

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
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool, format: OutputFormat) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(format),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn tamper_checksum(pool: &DbPool) -> Option<(String, i64, String)> {
    let row: Option<(String, i64, String)> = sqlx::query_as(
        "SELECT extension_id, version::bigint, checksum FROM extension_migrations \
         ORDER BY extension_id, version LIMIT 1",
    )
    .fetch_optional(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    let (ext, version, original) = row?;
    sqlx::query(
        "UPDATE extension_migrations SET checksum = 'cov-tampered-checksum' \
         WHERE extension_id = $1 AND version = $2",
    )
    .bind(&ext)
    .bind(version)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    Some((ext, version, original))
}

async fn restore_checksum(pool: &DbPool, ext: &str, version: i64, checksum: &str) {
    sqlx::query(
        "UPDATE extension_migrations SET checksum = $3 \
         WHERE extension_id = $1 AND version = $2",
    )
    .bind(ext)
    .bind(version)
    .bind(checksum)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
}

#[tokio::test]
async fn repair_dry_run_reports_clean_state() {
    let pool = pool().await;
    db::execute(parse(&["migrate-repair"]), &ctx(&pool, OutputFormat::Table))
        .await
        .unwrap();
    db::execute(
        parse(&["migrate-repair", "--json"]),
        &ctx(&pool, OutputFormat::Json),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn repair_unknown_extension_errors() {
    let pool = pool().await;
    let err = db::execute(
        parse(&["migrate-repair", "no-such-extension"]),
        &ctx(&pool, OutputFormat::Json),
    )
    .await
    .unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}

#[tokio::test]
async fn repair_dry_run_reports_tampered_checksum_drift() {
    let pool = pool().await;

    db::execute(
        parse(&["migrate-repair", "--apply"]),
        &ctx(&pool, OutputFormat::Json),
    )
    .await
    .unwrap();

    let Some((ext, version, original)) = tamper_checksum(&pool).await else {
        return;
    };

    let text_run = db::execute(
        parse(&["migrate-repair", &ext]),
        &ctx(&pool, OutputFormat::Table),
    )
    .await;
    let json_run = db::execute(
        parse(&["migrate-repair", &ext, "--json"]),
        &ctx(&pool, OutputFormat::Json),
    )
    .await;

    restore_checksum(&pool, &ext, version, &original).await;
    text_run.unwrap();
    json_run.unwrap();

    let (checksum,): (String,) = sqlx::query_as(
        "SELECT checksum FROM extension_migrations WHERE extension_id = $1 AND version = $2",
    )
    .bind(&ext)
    .bind(version)
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert_eq!(checksum, original);
}
