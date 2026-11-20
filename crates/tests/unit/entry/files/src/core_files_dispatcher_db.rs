//! DB-backed tests for the `core files` dispatcher.
//!
//! The list/show/search/stats arms run on a database-scoped context; the
//! remaining arms are guarded and must refuse it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::CommandError;
use systemprompt_files_entry::cli::{self as files, FilesCommands};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: FilesCommands,
}

fn parse(args: &[&str]) -> FilesCommands {
    Harness::try_parse_from(args.iter().copied()).unwrap().cmd
}

fn ctx(pool: &DbPool) -> CommandContext {
    crate::common::with_database(pool)
}

async fn profile_ctx() -> CommandContext {
    crate::common::with_app(
        crate::common::json_cli(),
        EnvOverrides::default(),
        test_app_context(&ensure_test_bootstrap().database_url).await,
    )
}

#[tokio::test]
async fn read_only_file_arms_run_against_the_database() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    files::execute(parse(&["files", "list"]), &ctx)
        .await
        .unwrap();
    files::execute(parse(&["files", "list", "--limit", "5"]), &ctx)
        .await
        .unwrap();
    files::execute(parse(&["files", "stats"]), &ctx)
        .await
        .unwrap();
    files::execute(parse(&["files", "search", "covquery"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_arm_reports_an_unknown_file_id() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    let err = files::execute(parse(&["files", "show", "cov_absent_file"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Failed to show file"));
}

#[tokio::test]
async fn profile_only_file_arms_are_refused_under_a_database_scope() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["files", "validate", "/tmp/cov-absent-file.png"],
        vec!["files", "config"],
        vec!["files", "delete", "cov_absent_file", "--yes"],
        vec!["files", "ai", "list"],
    ] {
        let err = files::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("requires full profile context"),
            "{args:?}: {err:#}"
        );
    }
}

#[tokio::test]
async fn file_validation_reports_supported_and_rejected_files_without_uploading_them() {
    let temp = tempfile::tempdir().unwrap();
    let accepted = temp.path().join("operator-notes.txt");
    let rejected = temp.path().join("unknown-payload.exe");
    std::fs::write(&accepted, "review notes\n").unwrap();
    std::fs::write(&rejected, [0_u8, 159, 146, 150]).unwrap();

    let ctx = profile_ctx().await;
    files::execute(
        parse(&["files", "validate", accepted.to_str().unwrap()]),
        &ctx,
    )
    .await
    .expect("an allowed text file produces a validation report");
    let invalid = files::execute(
        parse(&["files", "validate", rejected.to_str().unwrap()]),
        &ctx,
    )
    .await
    .expect_err("a rejected file is reported and the command exits non-zero");
    assert!(format!("{invalid:#}").contains("failed validation"));
    let report = invalid
        .downcast_ref::<CommandError>()
        .and_then(CommandError::output)
        .expect("the failure carries the validation report");
    assert_eq!(report.title(), Some("File Invalid"));

    let missing = files::execute(
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
