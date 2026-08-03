//! DB-backed tests for the `core content analytics` subtree.
//!
//! Clicks, campaign, and journey each project a different query; the
//! dispatcher arms and their empty-result paths were never entered.

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

#[tokio::test]
async fn the_journey_graph_renders_for_the_default_window() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    core::execute(parse(&["content", "analytics", "journey"]), &ctx)
        .await
        .unwrap();
    core::execute(
        parse(&["content", "analytics", "journey", "--limit", "5"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn click_history_for_an_unknown_link_yields_an_empty_projection() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    core::execute(
        parse(&[
            "content",
            "analytics",
            "clicks",
            "cov_absent_link",
            "--limit",
            "5",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn campaign_analytics_reject_an_unknown_campaign() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = core::execute(
        parse(&["content", "analytics", "campaign", "cov_absent_campaign"]),
        &ctx,
    )
    .await
    .unwrap_err();

    assert!(
        format!("{err:#}").contains("cov_absent_campaign"),
        "{err:#}"
    );
}

#[tokio::test]
async fn pagination_offsets_are_accepted_by_the_journey_projection() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    core::execute(
        parse(&[
            "content",
            "analytics",
            "journey",
            "--limit",
            "1",
            "--offset",
            "1",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}
