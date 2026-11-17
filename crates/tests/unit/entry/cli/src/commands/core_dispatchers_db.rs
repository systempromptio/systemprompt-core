//! DB-backed tests driving the `core files` dispatcher.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::files::{self, FilesCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

#[derive(Debug, Parser)]
struct FilesHarness {
    #[command(subcommand)]
    cmd: FilesCommands,
}

fn parse_files(args: &[&str]) -> FilesCommands {
    FilesHarness::try_parse_from(std::iter::once("files").chain(args.iter().copied()))
        .unwrap()
        .cmd
}


fn ctx(pool: &DbPool) -> CommandContext {
    crate::boot_ctx::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        test_database_url(),
    )
}

#[tokio::test]
async fn files_read_commands_run_database_scoped() {
    let pool = test_db_pool().await;
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
    let pool = test_db_pool().await;
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
