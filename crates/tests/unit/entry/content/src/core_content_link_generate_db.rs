//! App-context tests driving `core content link generate` against a real
//! database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_content_entry::cli::content::{self, ContentCommands};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_test_fixtures::{test_app_context, test_database_url};

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

#[tokio::test]
async fn generate_redirect_only_link() {
    let ctx = ctx().await;
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
    let ctx = ctx().await;
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
    let ctx = ctx().await;
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
    let ctx = ctx().await;
    let err = content::execute(parse(&["link", "generate", "--url", ""]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("URL is required"), "{err:#}");
}
