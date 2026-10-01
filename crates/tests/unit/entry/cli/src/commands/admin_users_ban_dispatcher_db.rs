//! DB-backed tests for the `admin users ban` subtree.
//!
//! Reads run under a database scope; the write arms are guarded and must
//! refuse it. Both halves of that contract were untested.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::{self, AdminCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AdminCommands,
}

fn parse(args: &[&str]) -> AdminCommands {
    Harness::try_parse_from(std::iter::once("admin").chain(args.iter().copied()))
        .unwrap()
        .cmd
}


fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        test_database_url(),
    )
}

#[tokio::test]
async fn ban_listing_runs_with_and_without_a_source_filter() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    admin::execute(parse(&["users", "ban", "list"]), &ctx)
        .await
        .unwrap();
    admin::execute(parse(&["users", "ban", "list", "--limit", "5"]), &ctx)
        .await
        .unwrap();
    admin::execute(
        parse(&["users", "ban", "list", "--source", "cov_no_such_source"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn checking_an_unbanned_address_reports_it_as_clear() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    admin::execute(parse(&["users", "ban", "check", "198.51.100.7"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn ban_writes_are_refused_under_a_database_scope() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec![
            "users",
            "ban",
            "add",
            "198.51.100.9",
            "--reason",
            "coverage",
        ],
        vec!["users", "ban", "remove", "198.51.100.9", "--yes"],
        vec!["users", "ban", "cleanup"],
    ] {
        let err = admin::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("Write operations require full profile"),
            "{args:?}: {err:#}"
        );
    }
}
