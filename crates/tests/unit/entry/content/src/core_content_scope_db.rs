//! DB-backed tests driving the `core content` dispatcher under a database
//! scope: the read commands run, the mutating ones require a full profile.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_content_entry::cli::content::{self, ContentCommands};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

#[derive(Debug, Parser)]
struct ContentHarness {
    #[command(subcommand)]
    cmd: ContentCommands,
}

fn parse_content(args: &[&str]) -> ContentCommands {
    ContentHarness::try_parse_from(std::iter::once("content").chain(args.iter().copied()))
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
async fn content_read_commands_run_database_scoped() {
    let pool = test_db_pool().await;
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
    content::execute(parse_content(&["analytics", "journey"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn content_mutating_commands_require_full_profile() {
    let pool = test_db_pool().await;
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
