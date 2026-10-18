#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{DbCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{DisposableDb, ensure_test_bootstrap};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: DbCommands,
}

fn command(args: &[&str]) -> DbCommands {
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
        .unwrap()
        .command
}

fn text_config() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn json_config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

async fn standalone(db: &DisposableDb, config: CliConfig) -> CommandContext {
    CommandContext::with_database(
        config,
        EnvOverrides::default(),
        DatabaseContext::from_pool(db.test_pool().await),
        db.url().to_owned(),
    )
}

#[tokio::test]
async fn migrate_installs_an_empty_database_and_leaves_nothing_pending() {
    ensure_test_bootstrap();
    let db = DisposableDb::empty("cli_migrate_empty").await;
    let ctx = standalone(&db, text_config()).await;

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("a fresh database installs");
    execute(command(&["migrate"]), &ctx)
        .await
        .expect("a second migrate is a no-op");
    execute(command(&["migrate-plan", "--require-rolling-safe"]), &ctx)
        .await
        .expect("an installed database has no breaking pending migration");

    db.drop_now().await;
}

#[tokio::test]
async fn migrate_renders_json_output() {
    ensure_test_bootstrap();
    let db = DisposableDb::empty("cli_migrate_json").await;
    let ctx = standalone(&db, json_config()).await;

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("json migrate succeeds");
    execute(command(&["migrate-plan", "--json"]), &ctx)
        .await
        .expect("json plan succeeds");

    db.drop_now().await;
}

#[tokio::test]
async fn migrate_plan_for_an_unknown_extension_is_refused() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_migrate_plan_unknown").await;
    let ctx = standalone(&db, text_config()).await;

    let err = execute(command(&["migrate-plan", "no-such-extension"]), &ctx)
        .await
        .expect_err("an unknown extension has no plan");
    assert!(err.to_string().contains("not found"), "{err:#}");

    db.drop_now().await;
}

#[tokio::test]
async fn migrate_down_for_an_unknown_extension_fails() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_migrate_down_unknown").await;
    let ctx = standalone(&db, text_config()).await;

    let result = execute(command(&["migrate-down", "no-such-extension", "1"]), &ctx).await;
    assert!(result.is_err());

    db.drop_now().await;
}

#[tokio::test]
async fn profile_migrate_runs_against_the_profile_database() {
    ensure_test_bootstrap();
    let ctx = CommandContext::new(text_config(), EnvOverrides::default());

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("the profile database is already installed, so migrate is a no-op");
}

#[tokio::test]
async fn verbose_profile_migrate_reports_its_target_and_stays_a_no_op() {
    ensure_test_bootstrap();
    let ctx = CommandContext::new(
        text_config().with_verbosity(systemprompt_cli::VerbosityLevel::Verbose),
        EnvOverrides::default(),
    );

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("a verbose migrate against the installed profile database succeeds");
    execute(command(&["migrate-plan", "--require-rolling-safe"]), &ctx)
        .await
        .expect("the installed profile database has nothing pending");
}
