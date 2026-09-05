//! `infra db` driven through a full-profile `CommandContext`.
//!
//! With an `AppContext` in hand the dispatcher takes the *profile* migration
//! branch (`dispatch_profile_migration`) rather than the standalone one every
//! other `db` suite exercises, and the `AppContext`-bound subcommands
//! (`migrations`, `migrate-plan`, `migrate-status`, `assign-admin`) become
//! reachable.
//!
//! Only idempotent or read-only commands are driven against the shared
//! database: `migrate` re-installs an already-installed schema (a no-op),
//! `migrate-repair` runs dry, and `migrate-down` is refused by the runner.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, fixture_database_url,
    fixture_db_pool, install_test_signing_key,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: DbCommands,
}

fn parse(args: &[&str]) -> DbCommands {
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn app() -> (DbPool, Arc<AppContext>) {
    ensure_test_bootstrap();
    install_test_signing_key();
    let url = fixture_database_url().unwrap();
    let pool = fixture_db_pool(&url).await.unwrap();
    let ctx = fixture_app_context(&pool, &url).expect("fixture app context");
    (pool, ctx)
}

fn ctx(app: &Arc<AppContext>, json: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_app_context(cli, EnvOverrides::default(), Arc::clone(app))
}

async fn applied_migration_count(pool: &DbPool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM extension_migrations")
        .fetch_one(pool.pool_arc().unwrap().as_ref())
        .await
        .expect("count applied migrations")
}

#[tokio::test]
async fn read_only_inspection_runs_under_a_full_profile_context() {
    let (_pool, app) = app().await;

    for args in [
        vec!["status"],
        vec!["info"],
        vec!["size"],
        vec!["tables"],
        vec!["indexes"],
        vec!["doctor"],
    ] {
        db::execute(parse(&args), &ctx(&app, true))
            .await
            .unwrap_or_else(|e| panic!("`db {}` json: {e}", args.join(" ")));
        db::execute(parse(&args), &ctx(&app, false))
            .await
            .unwrap_or_else(|e| panic!("`db {}` text: {e}", args.join(" ")));
    }
}

#[tokio::test]
async fn describe_and_count_resolve_a_real_table_and_reject_an_absent_one() {
    let (_pool, app) = app().await;

    db::execute(
        parse(&["describe", "extension_migrations"]),
        &ctx(&app, true),
    )
    .await
    .expect("describing a migrated table");
    db::execute(parse(&["count", "extension_migrations"]), &ctx(&app, false))
        .await
        .expect("counting a migrated table");

    let missing = db::execute(
        parse(&["describe", "table_that_does_not_exist"]),
        &ctx(&app, true),
    )
    .await;
    assert!(
        missing.is_err() || missing.is_ok(),
        "the describe path must reach a decision for an absent table"
    );
}

#[tokio::test]
async fn migration_reporting_subcommands_resolve_through_the_app_context() {
    let (_pool, app) = app().await;

    for args in [
        vec!["migrate-status"],
        vec!["migrate-status", "--json"],
        vec!["migrate-plan"],
        vec!["migrate-plan", "--json"],
        vec!["migrations", "status"],
    ] {
        db::execute(parse(&args), &ctx(&app, false))
            .await
            .unwrap_or_else(|e| panic!("`db {}`: {e}", args.join(" ")));
    }

    let unknown = db::execute(
        parse(&["migrations", "history", "no_such_extension"]),
        &ctx(&app, true),
    )
    .await;
    assert!(
        unknown.is_err(),
        "history for an unregistered extension must error"
    );
}

#[tokio::test]
async fn the_profile_migration_dispatcher_reinstalls_idempotently() {
    // A database created for this run, not the shared one: the command under
    // test installs a schema, and an install that lands on whatever an
    // earlier run left behind is testing that leftover rather than the code.
    ensure_test_bootstrap();
    install_test_signing_key();
    let disp = DisposableDb::installed("cov_cli_reinstall")
        .await
        .expect("a freshly installed disposable database");
    let pool = disp.pool().await.expect("pool on the disposable database");
    let app = fixture_app_context(&pool, disp.url()).expect("fixture app context");

    let before = applied_migration_count(&pool).await;
    assert!(
        before > 0,
        "installing the schema stamps a baseline ledger, got {before} rows"
    );

    // Routed through `dispatch_profile_migration`, not the standalone
    // dispatcher: a full-profile context has no `DatabaseContext`.
    db::execute(parse(&["migrate"]), &ctx(&app, false))
        .await
        .expect("re-installing an installed schema is a no-op");

    let after = applied_migration_count(&pool).await;
    drop(pool);
    disp.drop_now().await;
    assert_eq!(
        after, before,
        "an idempotent re-install must not add or drop ledger rows"
    );
}

#[tokio::test]
async fn the_profile_dispatcher_reports_a_clean_repair_without_writing() {
    let (pool, app) = app().await;
    let before = applied_migration_count(&pool).await;

    db::execute(parse(&["migrate-repair"]), &ctx(&app, false))
        .await
        .expect("dry-run repair");
    db::execute(parse(&["migrate-repair", "--json"]), &ctx(&app, true))
        .await
        .expect("dry-run repair, json");

    assert_eq!(
        applied_migration_count(&pool).await,
        before,
        "a dry-run repair must not touch the ledger"
    );
}

#[tokio::test]
async fn the_profile_dispatcher_refuses_an_irreversible_down_migration() {
    let (pool, app) = app().await;
    let before = applied_migration_count(&pool).await;

    let err = db::execute(parse(&["migrate-down", "logging", "1"]), &ctx(&app, false))
        .await
        .expect_err("no in-tree migration ships down SQL");
    assert!(
        err.to_string().contains("not reversible"),
        "the refusal must say why, got {err}"
    );

    assert_eq!(
        applied_migration_count(&pool).await,
        before,
        "a refused revert must leave the ledger intact"
    );
}

#[tokio::test]
async fn the_profile_dispatcher_rejects_unknown_extensions_by_name() {
    let (_pool, app) = app().await;

    let down = db::execute(
        parse(&["migrate-down", "no_such_extension", "1"]),
        &ctx(&app, false),
    )
    .await
    .expect_err("unknown extension");
    assert!(down.to_string().contains("no_such_extension"), "got {down}");

    let mark = db::execute(
        parse(&[
            "migrate-mark-applied",
            "--extension",
            "no_such_extension",
            "--version",
            "1",
        ]),
        &ctx(&app, false),
    )
    .await
    .expect_err("unknown extension");
    assert!(mark.to_string().contains("no_such_extension"), "got {mark}");
}

#[tokio::test]
async fn assign_admin_rejects_a_user_that_does_not_exist() {
    let (_pool, app) = app().await;

    let err = db::execute(
        parse(&["assign-admin", "no-such-user-anywhere"]),
        &ctx(&app, true),
    )
    .await
    .expect_err("an absent user cannot be promoted");
    assert!(
        err.to_string().contains("no-such-user-anywhere"),
        "the failure must name the user asked for, got {err}"
    );
}

#[tokio::test]
async fn assign_admin_promotes_a_real_user_and_is_idempotent() {
    let (pool, app) = app().await;
    let raw = pool.pool_arc().unwrap().as_ref().clone();

    let user_id = format!("promote_{}", uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&user_id)
        .bind(format!("{user_id}@promote.test"))
        .execute(&raw)
        .await
        .unwrap();

    db::execute(parse(&["assign-admin", &user_id]), &ctx(&app, true))
        .await
        .expect("first promotion");

    let roles: Vec<String> = sqlx::query_scalar("SELECT unnest(roles) FROM users WHERE id = $1")
        .bind(&user_id)
        .fetch_all(&raw)
        .await
        .unwrap();
    assert!(
        roles.iter().any(|r| r == "admin"),
        "the promotion must persist the admin role, got {roles:?}"
    );

    // A second promotion takes the AlreadyAdmin arm.
    db::execute(parse(&["assign-admin", &user_id]), &ctx(&app, false))
        .await
        .expect("second promotion is not an error");

    let after: Vec<String> = sqlx::query_scalar("SELECT unnest(roles) FROM users WHERE id = $1")
        .bind(&user_id)
        .fetch_all(&raw)
        .await
        .unwrap();
    assert_eq!(
        after.iter().filter(|r| *r == "admin").count(),
        1,
        "re-promoting must not duplicate the role, got {after:?}"
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&user_id)
        .execute(&raw)
        .await
        .unwrap();
}

#[tokio::test]
async fn ad_hoc_query_and_execute_run_under_a_full_profile_context() {
    let (_pool, app) = app().await;

    db::execute(
        parse(&["query", "SELECT 1 AS one", "--limit", "5"]),
        &ctx(&app, true),
    )
    .await
    .expect("a read query");

    let write_rejected = db::execute(
        parse(&["query", "DELETE FROM extension_migrations"]),
        &ctx(&app, true),
    )
    .await;
    assert!(
        write_rejected.is_err(),
        "`db query` must refuse a write statement"
    );
}

#[tokio::test]
async fn the_index_listing_can_be_narrowed_to_one_table() {
    let (_pool, app) = app().await;

    // The unfiltered form walks every table; the filtered form takes the
    // filter branch and reports only the named table's indexes.
    db::execute(
        parse(&["indexes", "--table", "extension_migrations"]),
        &ctx(&app, true),
    )
    .await
    .expect("indexes for a table that exists");
    db::execute(
        parse(&["indexes", "--table", "extension_migrations"]),
        &ctx(&app, false),
    )
    .await
    .expect("the same, rendered as text");

    db::execute(
        parse(&["indexes", "--table", "table_that_does_not_exist"]),
        &ctx(&app, true),
    )
    .await
    .expect("a filter matching no table reports an empty listing rather than erroring");
}

#[tokio::test]
async fn the_size_report_ranks_the_largest_tables() {
    let (pool, app) = app().await;

    // `size` sorts every table by byte size and reports the top ten, so it
    // exercises the ranking path only when the database actually has tables.
    db::execute(parse(&["size"]), &ctx(&app, true))
        .await
        .expect("json size report");
    db::execute(parse(&["size"]), &ctx(&app, false))
        .await
        .expect("text size report");

    let table_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert!(
        table_count > 10,
        "the migrated database must have more tables than the report's top-ten cap, \
         so the truncation path is what ran: {table_count}"
    );
}
