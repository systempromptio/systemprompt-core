//! DB-backed test for the `core` dispatcher's database-scope guard: a
//! `--database-url` invocation refuses every `core` subgroup that needs a full
//! profile context.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

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
async fn profile_only_groups_are_refused_under_a_database_scope() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["skills", "list"],
        vec!["hooks", "list"],
        vec!["contexts", "list"],
    ] {
        let err = format!("{:#}", core::execute(parse(&args), &ctx).await.unwrap_err());
        assert!(
            err.contains("requires full profile context") || err.contains("does not support"),
            "{args:?}: {err}"
        );
    }
}
