//! App-context tests driving `core content link generate` against a real
//! database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::content::{self, ContentCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};

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

#[tokio::test]
async fn generate_redirect_only_link() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    content::execute(
        parse(&[
            "link",
            "generate",
            "--url",
            "https://example.com/redirect-only",
            "--link-type",
            "redirect",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn generate_link_with_full_utm_params() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    content::execute(
        parse(&[
            "link",
            "generate",
            "--url",
            "https://example.com/utm",
            "--link-type",
            "both",
            "--campaign-name",
            "spring",
            "--utm-source",
            "newsletter",
            "--utm-medium",
            "email",
            "--utm-campaign",
            "spring2026",
            "--utm-term",
            "governance",
            "--utm-content",
            "cta",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn generate_utm_link_without_params() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    content::execute(
        parse(&[
            "link",
            "generate",
            "--url",
            "https://example.com/no-utm",
            "--link-type",
            "utm",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn generate_empty_url_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = content::execute(parse(&["link", "generate", "--url", ""]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("URL is required"), "{err:#}");
}
