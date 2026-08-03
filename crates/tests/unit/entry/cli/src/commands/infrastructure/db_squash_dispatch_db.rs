//! DB-backed tests for the `infra db migrate-squash` text renderer.
//!
//! The JSON dry-run is covered in `db_commands_db`; this drives the same
//! command with a text-output config so the human-readable renderer and its
//! applied/dry-run branches are exercised against a real migration plan.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
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

async fn text_ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

#[tokio::test]
async fn text_output_dry_run_renders_a_plan_for_a_registered_extension() {
    let pool = fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap();
    let ctx = text_ctx(&pool).await;

    db::execute(
        parse(&["migrate-squash", "--extension", "logging", "--through", "1"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn text_output_reports_an_unknown_extension() {
    let pool = fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap();
    let ctx = text_ctx(&pool).await;

    let err = db::execute(
        parse(&[
            "migrate-squash",
            "--extension",
            "no_such_extension",
            "--through",
            "1",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("not found in registry"));
}
