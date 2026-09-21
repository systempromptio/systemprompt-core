//! DB-backed tests for the `core files` dispatcher.
//!
//! The list/show/search/stats arms run on a database-scoped context; the
//! remaining arms are guarded and must refuse it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
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

fn profile_ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[tokio::test]
async fn read_only_file_arms_run_against_the_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    core::execute(parse(&["files", "list"]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["files", "list", "--limit", "5"]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["files", "stats"]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["files", "search", "covquery"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_arm_reports_an_unknown_file_id() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = core::execute(parse(&["files", "show", "cov_absent_file"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Failed to show file"));
}

#[tokio::test]
async fn profile_only_file_arms_are_refused_under_a_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["files", "validate", "/tmp/cov-absent-file.png"],
        vec!["files", "config"],
        vec!["files", "delete", "cov_absent_file", "--yes"],
        vec!["files", "ai", "list"],
    ] {
        let err = core::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("requires full profile context"),
            "{args:?}: {err:#}"
        );
    }
}

#[tokio::test]
async fn file_validation_reports_supported_and_rejected_files_without_uploading_them() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let temp = tempfile::tempdir().unwrap();
    let accepted = temp.path().join("operator-notes.txt");
    let rejected = temp.path().join("unknown-payload.exe");
    std::fs::write(&accepted, "review notes\n").unwrap();
    std::fs::write(&rejected, [0_u8, 159, 146, 150]).unwrap();

    let ctx = profile_ctx();
    core::execute(
        parse(&["files", "validate", accepted.to_str().unwrap()]),
        &ctx,
    )
    .await
    .expect("an allowed text file produces a validation report");
    core::execute(
        parse(&["files", "validate", rejected.to_str().unwrap()]),
        &ctx,
    )
    .await
    .expect("a rejected file is reported as invalid rather than uploaded or rejected as a command error");

    let missing = core::execute(
        parse(&[
            "files",
            "validate",
            temp.path().join("missing.txt").to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .expect_err("a missing path cannot be validated");
    assert!(format!("{missing:#}").contains("File not found"));
}
