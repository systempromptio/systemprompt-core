//! DB-backed tests driving the `core content` and `core files` dispatchers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::content::{self, ContentCommands};
use systemprompt_cli::core::files::{self, FilesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct ContentHarness {
    #[command(subcommand)]
    cmd: ContentCommands,
}

#[derive(Debug, Parser)]
struct FilesHarness {
    #[command(subcommand)]
    cmd: FilesCommands,
}

fn parse_content(args: &[&str]) -> ContentCommands {
    ContentHarness::try_parse_from(std::iter::once("content").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn parse_files(args: &[&str]) -> FilesCommands {
    FilesHarness::try_parse_from(std::iter::once("files").chain(args.iter().copied()))
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
async fn content_read_commands_run_database_scoped() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    content::execute(parse_content(&["list", "--limit", "5"]), &ctx)
        .await
        .unwrap();
    content::execute(parse_content(&["search", "anything"]), &ctx)
        .await
        .unwrap();
    content::execute(
        parse_content(&["status", "--source", "no-such-source"]),
        &ctx,
    )
    .await
    .unwrap();
    content::execute(parse_content(&["popular", "no-such-source"]), &ctx)
        .await
        .unwrap();
    content::execute(parse_content(&["analytics", "journey"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn content_mutating_commands_require_full_profile() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["edit", "whatever"],
        vec!["delete", "whatever", "--yes"],
        vec!["verify", "whatever"],
        vec!["link", "list"],
        vec!["files", "list"],
    ] {
        let err = content::execute(parse_content(&args), &ctx)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("requires full profile context"),
            "{args:?}: {err}"
        );
    }
}

#[tokio::test]
async fn files_read_commands_run_database_scoped() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    files::execute(parse_files(&["list"]), &ctx).await.unwrap();
    files::execute(parse_files(&["search", "%.png"]), &ctx)
        .await
        .unwrap();
    files::execute(parse_files(&["stats"]), &ctx).await.unwrap();

    let err = files::execute(parse_files(&["show", "no-such-file"]), &ctx)
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn files_profile_commands_refuse_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["upload", "--context", "ctx1", "/tmp/nope.png"],
        vec!["delete", "some-file", "--yes"],
        vec!["config"],
        vec!["ai", "list"],
    ] {
        let err = files::execute(parse_files(&args), &ctx).await.unwrap_err();
        assert!(
            err.to_string().contains("requires full profile context"),
            "{args:?}: {err}"
        );
    }
}
