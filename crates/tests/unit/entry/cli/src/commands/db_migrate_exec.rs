#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{DbCommands, execute};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
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
    crate::boot_ctx::with_database(
        config,
        EnvOverrides::default(),
        DatabaseContext::from_pool(db.test_pool().await),
        db.url().to_owned(),
    )
}

fn profile_driven(db: &DisposableDb, config: CliConfig) -> CommandContext {
    crate::boot_ctx::new_on_database(config, EnvOverrides::default(), db.url())
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
    execute(command(&["migrate-plan"]), &ctx)
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
    let db = DisposableDb::with_schema("cli_profile_migrate").await;
    let ctx = profile_driven(&db, text_config());

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("the profile database is already installed, so migrate is a no-op");

    db.drop_now().await;
}

#[tokio::test]
async fn verbose_profile_migrate_reports_its_target_and_stays_a_no_op() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_profile_migrate_verbose").await;
    systemprompt_test_fixtures::seed_fixture_system_admin(&db.test_pool().await)
        .await
        .expect("seed the profile's system admin");
    let ctx = profile_driven(
        &db,
        text_config().with_verbosity(systemprompt_extension::cli::VerbosityLevel::Verbose),
    );

    execute(command(&["migrate"]), &ctx)
        .await
        .expect("a verbose migrate against the installed profile database succeeds");
    execute(command(&["migrate-plan", "--require-rolling-safe"]), &ctx)
        .await
        .expect("the installed profile database has nothing pending");

    db.drop_now().await;
}

async fn record_ahead(db: &DisposableDb) {
    let pool = db.test_pool().await;
    sqlx::query(
        "INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
         VALUES ('logging:990', 'logging', 990, 'unreleased_change', 'unknown-checksum')",
    )
    .execute(pool.pool().as_ref())
    .await
    .expect("record a migration this binary does not know");
}

#[tokio::test]
async fn migrate_succeeds_with_a_warning_when_a_member_is_only_ahead() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_migrate_ahead").await;
    record_ahead(&db).await;
    let ctx = standalone(&db, text_config()).await;

    let output = execute(command(&["migrate"]), &ctx)
        .await
        .expect("an unmarked ahead row exits 0");
    let notes = format!("{:?}", output.notes());
    assert!(notes.contains("warning"), "{notes}");
    assert!(
        notes.contains("logging") && notes.contains("990") && notes.contains("unreleased_change"),
        "the warning names the ahead row: {notes}"
    );

    db.drop_now().await;
}

#[tokio::test]
async fn migrate_refuses_an_ahead_row_marked_breaking_older() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_migrate_breaking").await;
    record_ahead(&db).await;
    let pool = db.test_pool().await;
    sqlx::query(
        "INSERT INTO extension_migration_compat (extension_id, version, breaks_older) \
         VALUES ('logging', 990, TRUE)",
    )
    .execute(pool.pool().as_ref())
    .await
    .expect("mark the ahead migration breaking");
    let ctx = standalone(&db, text_config()).await;

    let err = execute(command(&["migrate"]), &ctx)
        .await
        .expect_err("a breaking-older row fails the migrate");
    let text = err.to_string();
    assert!(text.contains("@compat: breaking"), "{text}");
    assert!(text.contains("logging 990 unreleased_change"), "{text}");

    db.drop_now().await;
}

#[tokio::test]
async fn migrate_refuses_when_a_member_is_held_back_and_names_it_with_its_reason() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cli_migrate_held_back").await;
    record_ahead(&db).await;
    let pool = db.test_pool().await;
    sqlx::query(
        "DELETE FROM extension_migrations WHERE extension_id = 'logging' AND version = \
         (SELECT MAX(version) FROM extension_migrations WHERE extension_id = 'logging' \
         AND version < 990)",
    )
    .execute(pool.pool().as_ref())
    .await
    .expect("leave one of this binary's logging migrations pending");
    let ctx = standalone(&db, text_config()).await;

    let err = execute(command(&["migrate"]), &ctx)
        .await
        .expect_err("a held-back member fails the migrate");
    let text = err.to_string();
    assert!(text.contains("held back 1 extension(s)"), "{text}");
    assert!(
        text.contains("logging: the database records migration(s) this binary does not know: 990 unreleased_change"),
        "{text}"
    );
    let installed = text
        .lines()
        .find_map(|line| line.strip_prefix("Installed: "))
        .expect("the installed members are listed");
    assert!(
        !installed.split(", ").any(|id| id == "logging"),
        "a held-back member is never listed as installed: {installed}"
    );
    assert!(installed.split(", ").any(|id| id == "users"), "{installed}");

    db.drop_now().await;
}
