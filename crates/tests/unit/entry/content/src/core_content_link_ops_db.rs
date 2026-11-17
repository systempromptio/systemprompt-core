//! App-context tests driving `core content link show`, `performance`, and
//! `delete` against a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_content_entry::cli::content::{self, ContentCommands};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
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


async fn ctx() -> CommandContext {
    let url = test_database_url();
    crate::common::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        test_app_context(&url).await,
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

    let raw = pool.pool();
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
    let pool = test_db_pool().await;
    let ctx = ctx().await;
    let (_id, short_code) = seed_link(&pool, &ctx).await;

    content::execute(parse(&["link", "show", &short_code]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_unknown_short_code_errors() {
    let ctx = ctx().await;
    let missing = format!("nope{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "show", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}

#[tokio::test]
async fn performance_reports_zero_click_link() {
    let pool = test_db_pool().await;
    let ctx = ctx().await;
    let (id, _short_code) = seed_link(&pool, &ctx).await;

    content::execute(parse(&["link", "performance", &id]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn performance_unknown_link_errors() {
    let ctx = ctx().await;
    let missing = format!("missing-{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "performance", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}

#[tokio::test]
async fn delete_with_yes_removes_link() {
    let pool = test_db_pool().await;
    let ctx = ctx().await;
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
    let ctx = ctx().await;
    let missing = format!("missing-{}", Uuid::new_v4().simple());
    let err = content::execute(parse(&["link", "delete", &missing, "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Link not found"), "{err}");
}
