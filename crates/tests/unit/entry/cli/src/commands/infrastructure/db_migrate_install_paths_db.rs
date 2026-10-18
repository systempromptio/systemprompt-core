//! `infra db migrate` and `migrate-plan` against a disposable database whose
//! ledger was edited: an install refused on checksum drift carries the repair
//! hint, `--repair-drift` heals and retries, and a plan with a pending
//! `@compat: breaking` migration renders and fails `--require-rolling-safe`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, VerbosityLevel};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{DisposableDb, ensure_test_bootstrap};

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

async fn installed(name: &str) -> (DisposableDb, String) {
    let db = DisposableDb::empty(name).await;
    let url = db.url().to_owned();
    db::execute(parse(&["migrate"]), &ctx(&url, false).await)
        .await
        .unwrap();
    (db, url)
}

async fn ctx(url: &str, verbose: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if verbose {
        cli = cli.with_verbosity(VerbosityLevel::Verbose);
    }
    let db_ctx = DatabaseContext::from_url(url).await.unwrap();
    CommandContext::with_database(cli, EnvOverrides::default(), db_ctx, url.to_owned())
}

async fn exec(url: &str, sql: &str, extension: &str, version: i32) -> u64 {
    let pool = sqlx::PgPool::connect(url).await.unwrap();
    let affected = sqlx::query(sql)
        .bind(extension)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
    pool.close().await;
    affected
}

async fn checksum(url: &str, extension: &str, version: i32) -> String {
    let pool = sqlx::PgPool::connect(url).await.unwrap();
    let value: String = sqlx::query_scalar(
        "SELECT checksum FROM extension_migrations WHERE extension_id = $1 AND version = $2",
    )
    .bind(extension)
    .bind(version)
    .fetch_one(&pool)
    .await
    .unwrap();
    pool.close().await;
    value
}

#[tokio::test]
async fn drift_refuses_the_install_with_a_hint_and_repair_drift_heals_it() {
    ensure_test_bootstrap();
    let (db, url) = installed("cli_install_drift").await;
    let original = checksum(&url, "logging", 3).await;
    let tampered = exec(
        &url,
        "UPDATE extension_migrations SET checksum = 'cafebabecafebabe' \
         WHERE extension_id = $1 AND version = $2",
        "logging",
        3,
    )
    .await;
    assert_eq!(tampered, 1);

    let err = db::execute(parse(&["migrate"]), &ctx(&url, false).await)
        .await
        .expect_err("drift refuses a plain install");
    let text = format!("{err:#}");
    assert!(text.contains("Schema installation failed"), "{text}");
    assert!(text.contains("--repair-drift"), "the hint names the fix: {text}");

    db::execute(parse(&["migrate", "--repair-drift"]), &ctx(&url, true).await)
        .await
        .expect("repair-drift re-applies and retries");
    assert_eq!(checksum(&url, "logging", 3).await, original);

    db.drop_now().await;
}

#[tokio::test]
async fn a_pending_breaking_migration_is_planned_and_fails_the_rolling_safe_gate() {
    ensure_test_bootstrap();
    let (db, url) = installed("cli_plan_breaking").await;
    let removed = exec(
        &url,
        "DELETE FROM extension_migrations WHERE extension_id = $1 AND version = $2",
        "ai",
        43,
    )
    .await;
    assert_eq!(removed, 1, "the fixture must leave exactly one migration pending");

    db::execute(parse(&["migrate-plan"]), &ctx(&url, false).await)
        .await
        .expect("a text plan renders pending rows");
    db::execute(parse(&["migrate-plan", "ai", "--json"]), &ctx(&url, false).await)
        .await
        .expect("a json plan renders pending rows");

    let err = db::execute(
        parse(&["migrate-plan", "--require-rolling-safe"]),
        &ctx(&url, false).await,
    )
    .await
    .expect_err("a breaking pending migration is not rolling-safe");
    let text = err.to_string();
    assert!(text.contains("not rolling-safe"), "{text}");
    assert!(text.contains("ai:043"), "names the breaking migration: {text}");

    db.drop_now().await;
}
