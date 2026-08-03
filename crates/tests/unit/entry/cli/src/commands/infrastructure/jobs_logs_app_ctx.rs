//! `infra jobs` and `infra logs` subcommands gated behind a full profile.
//!
//! `jobs enable/disable`, `logs delete` and `logs cleanup` all bail out of a
//! `--database-url` invocation, so they are driven here through
//! `CommandContext::with_app_context`. `logs delete` is table-wide, so it runs
//! against a disposable database of its own.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::infrastructure::jobs::{self, JobsCommands};
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::JobRepository;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
    install_test_signing_key,
};

const KNOWN_JOB: &str = "content_prerender";

#[derive(Debug, Parser)]
struct JobsHarness {
    #[command(subcommand)]
    cmd: JobsCommands,
}

#[derive(Debug, Parser)]
struct LogsHarness {
    #[command(subcommand)]
    cmd: LogsCommands,
}

fn parse_jobs(args: &[&str]) -> JobsCommands {
    JobsHarness::try_parse_from(std::iter::once("jobs").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn parse_logs(args: &[&str]) -> LogsCommands {
    LogsHarness::try_parse_from(std::iter::once("logs").chain(args.iter().copied()))
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

#[tokio::test]
async fn disabling_then_enabling_a_registered_job_persists_the_flag_both_ways() {
    let (pool, app) = app().await;
    let repo = JobRepository::new(&pool).unwrap();
    // `set_enabled` is a bare UPDATE, so the schedule row has to exist before
    // the CLI toggle has anything to flip.
    repo.upsert_job(KNOWN_JOB, "0 0 * * * *", true)
        .await
        .expect("seed the schedule row");

    jobs::execute(parse_jobs(&["disable", KNOWN_JOB]), &ctx(&app, true))
        .await
        .expect("disable a registered job");
    let disabled = repo
        .find_job(KNOWN_JOB)
        .await
        .unwrap()
        .expect("the toggle must materialise a schedule row");
    assert!(
        !disabled.enabled,
        "`jobs disable` must persist enabled=false"
    );

    jobs::execute(parse_jobs(&["enable", KNOWN_JOB]), &ctx(&app, false))
        .await
        .expect("enable a registered job");
    let enabled = repo
        .find_job(KNOWN_JOB)
        .await
        .unwrap()
        .expect("the row must survive the second toggle");
    assert!(enabled.enabled, "`jobs enable` must persist enabled=true");
    assert_eq!(
        enabled.job_name, disabled.job_name,
        "the toggle must update the same row rather than inserting a second one"
    );
}

#[tokio::test]
async fn jobs_show_reports_a_registered_job_and_rejects_an_unknown_one() {
    let (_pool, app) = app().await;

    jobs::execute(parse_jobs(&["show", KNOWN_JOB]), &ctx(&app, true))
        .await
        .expect("json show");
    jobs::execute(parse_jobs(&["show", KNOWN_JOB]), &ctx(&app, false))
        .await
        .expect("text show");

    let err = jobs::execute(
        parse_jobs(&["show", "no_such_job_at_all"]),
        &ctx(&app, true),
    )
    .await
    .expect_err("an unregistered job has nothing to show");
    assert!(err.to_string().contains("no_such_job_at_all"), "got {err}");
}

#[tokio::test]
async fn jobs_history_renders_in_both_output_modes() {
    let (_pool, app) = app().await;

    jobs::execute(parse_jobs(&["history"]), &ctx(&app, true))
        .await
        .expect("json history");
    jobs::execute(parse_jobs(&["history"]), &ctx(&app, false))
        .await
        .expect("text history");
}

#[tokio::test]
async fn logs_cleanup_honours_its_retention_window() {
    let (pool, app) = app().await;
    let raw = pool.pool_arc().unwrap().as_ref().clone();

    let owner = format!("cleanup_owner_{}", uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&owner)
        .bind(format!("{owner}@cleanup.test"))
        .execute(&raw)
        .await
        .unwrap();

    // Every attribution column has to be populated: global reads of `logs`
    // (`infra logs export`) decode them as non-Option, so one NULL here fails
    // an unrelated suite.
    let recent_id = format!("recent_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id, session_id, trace_id) \
         VALUES ($1, now(), 'INFO', 'cli.cleanup', 'recent entry', $2, $3, $4)",
    )
    .bind(&recent_id)
    .bind(&owner)
    .bind(format!("sess_{recent_id}"))
    .bind(format!("trace_{recent_id}"))
    .execute(&raw)
    .await
    .unwrap();

    logs::execute(
        parse_logs(&["cleanup", "--keep-last-days", "3650", "-y"]),
        &ctx(&app, true),
    )
    .await
    .expect("cleanup with a long retention window");

    let survived: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM logs WHERE id = $1)")
        .bind(&recent_id)
        .fetch_one(&raw)
        .await
        .unwrap();
    assert!(
        survived,
        "a 10-year retention window must not delete a log written seconds ago"
    );

    sqlx::query("DELETE FROM logs WHERE id = $1")
        .bind(&recent_id)
        .execute(&raw)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&owner)
        .execute(&raw)
        .await
        .unwrap();
}

// `logs delete` truncates the whole table, so it gets a database nobody else
// is using.
#[tokio::test]
async fn logs_delete_clears_every_entry() {
    ensure_test_bootstrap();
    install_test_signing_key();

    let base_url = fixture_database_url().unwrap();
    let admin = fixture_db_pool(&base_url)
        .await
        .unwrap()
        .pool_arc()
        .unwrap()
        .as_ref()
        .clone();
    let name = format!("cov_cli_logsdel_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let (prefix, _) = base_url.rsplit_once('/').unwrap();
    let url = format!("{prefix}/{name}");

    {
        let pool = fixture_db_pool(&url).await.unwrap();
        let raw = pool.pool_arc().unwrap().as_ref().clone();
        systemprompt_database::install_extension_schemas_full(
            &systemprompt_extension::ExtensionRegistry::discover().unwrap(),
            pool.write(),
            &[],
            systemprompt_database::MigrationConfig::default(),
        )
        .await
        .expect("migrate the disposable database");

        let owner = "logdel_owner";
        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
            .bind(owner)
            .bind("logdel@test.invalid")
            .execute(&raw)
            .await
            .unwrap();
        for i in 0..3 {
            sqlx::query(
                "INSERT INTO logs (id, timestamp, level, module, message, user_id) \
                 VALUES ($1, now(), 'INFO', 'cli.delete', 'entry', $2)",
            )
            .bind(format!("del_{i}"))
            .bind(owner)
            .execute(&raw)
            .await
            .unwrap();
        }
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
            .fetch_one(&raw)
            .await
            .unwrap();
        assert_eq!(before, 3);

        let app = fixture_app_context(&pool, &url).expect("app context on the disposable db");
        logs::execute(parse_logs(&["delete", "-y"]), &ctx(&app, true))
            .await
            .expect("delete all logs");

        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
            .fetch_one(&raw)
            .await
            .unwrap();
        assert_eq!(after, 0, "`logs delete` must empty the table");
    }

    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await;
}
