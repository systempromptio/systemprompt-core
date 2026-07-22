//! App-context tests driving `core content link show`, `performance`, and
//! `delete` against a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::content::{self, ContentCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct ContentHarness {
    #[command(subcommand)]
    cmd: ContentCommands,
}

fn parse(args: &[&str]) -> ContentCommands {
    ContentHarness::try_parse_from(std::iter::once("content").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().unwrap();
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).unwrap(),
    )
}

async fn seed_link(pool: &DbPool, ctx: &CommandContext) -> (String, String) {
    let target = format!("https://example.com/ops-{}", Uuid::new_v4().simple());
    content::execute(
        parse(&[
            "link",
            "generate",
            "--url",
            &target,
            "--link-type",
            "redirect",
        ]),
        ctx,
    )
    .await
    .unwrap();

    let raw = pool.pool_arc().unwrap();
    let row: (String, String) =
        sqlx::query_as("SELECT id, short_code FROM campaign_links WHERE target_url = $1")
            .bind(&target)
            .fetch_one(raw.as_ref())
            .await
            .unwrap();
    row
}

#[tokio::test]
async fn show_reports_seeded_link() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let (_id, short_code) = seed_link(&pool, &ctx).await;

    content::execute(parse(&["link", "show", &short_code]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_unknown_short_code_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("nope{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "show", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}

#[tokio::test]
async fn performance_reports_zero_click_link() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let (id, _short_code) = seed_link(&pool, &ctx).await;

    content::execute(parse(&["link", "performance", &id]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn performance_unknown_link_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("missing-{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "performance", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}

#[tokio::test]
async fn delete_with_yes_removes_link() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let (id, short_code) = seed_link(&pool, &ctx).await;

    content::execute(parse(&["link", "delete", &id, "--yes"]), &ctx)
        .await
        .unwrap();

    let err = content::execute(parse(&["link", "show", &short_code]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}

#[tokio::test]
async fn delete_unknown_link_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("missing-{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "delete", &missing, "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}
