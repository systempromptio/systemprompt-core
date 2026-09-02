//! `infra services` subcommands that resolve an `AppContext`.
//!
//! `CommandContext::with_app_context` hands the dispatcher a real context, so
//! the status/stop/cleanup bodies run instead of failing at the profile gate a
//! `--database-url` context imposes. The fixture fleet is empty, which is the
//! shape the "nothing running" arms exist for.
//!
//! `start` and `serve` are deliberately not driven here: they spawn real
//! processes and bind real ports.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::infrastructure::services::{self, ServicesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
    install_test_signing_key,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ServicesCommands,
}

fn parse(args: &[&str]) -> ServicesCommands {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
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

// Scoped by name rather than compared whole: sibling suites in this subtree
// also drive commands that touch `services`, so a before/after snapshot of the
// entire table races them. The probe names below are ones no manifest
// declares, so a row bearing one can only have come from the command here.
async fn service_row_exists(pool: &DbPool, name: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM services WHERE name = $1)")
        .bind(name)
        .fetch_one(pool.pool_arc().unwrap().as_ref())
        .await
        .expect("read the service table")
}

const AGENT_PROBE: &str = "no-such-agent-anywhere";
const MCP_PROBE: &str = "no-such-mcp-server";

#[tokio::test]
async fn status_renders_the_fleet_in_json_text_detailed_and_health_modes() {
    let (_pool, app) = app().await;

    for args in [
        vec!["status"],
        vec!["status", "--detailed"],
        vec!["status", "--health"],
        vec!["status", "--detailed", "--health"],
    ] {
        services::execute(parse(&args), &ctx(&app, false))
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "`services {}` must render in text mode: {e}",
                    args.join(" ")
                )
            });
        services::execute(parse(&args), &ctx(&app, true))
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "`services {}` must render in json mode: {e}",
                    args.join(" ")
                )
            });
    }
}

#[tokio::test]
async fn stopping_an_empty_fleet_reports_nothing_stopped_and_changes_no_rows() {
    let (pool, app) = app().await;

    services::execute(parse(&["stop", "--agents"]), &ctx(&app, true))
        .await
        .expect("stopping zero agents is not an error");
    services::execute(parse(&["stop", "--mcp"]), &ctx(&app, true))
        .await
        .expect("stopping zero MCP servers is not an error");

    assert!(
        !service_row_exists(&pool, AGENT_PROBE).await
            && !service_row_exists(&pool, MCP_PROBE).await,
        "a stop over an empty target set must not invent service rows"
    );
}

#[tokio::test]
async fn stopping_an_empty_fleet_renders_the_human_form_too() {
    let (pool, app) = app().await;

    services::execute(parse(&["stop", "--agents", "--mcp"]), &ctx(&app, false))
        .await
        .expect("text-mode stop");

    assert!(!service_row_exists(&pool, AGENT_PROBE).await);
}

#[tokio::test]
async fn a_dry_run_cleanup_reports_without_stopping_anything() {
    let (pool, app) = app().await;
    let probe = format!("cleanup_probe_{}", uuid::Uuid::new_v4().simple());
    let raw = pool.pool_arc().unwrap().as_ref().clone();
    // A running row whose PID is this test process: `cleanup` classifies it as
    // live, so a dry run has to report it without removing it.
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, server_type, status, port, pid) \
         VALUES ('fixture', $1, 'cli-tests', 'internal', 'running', 65000, $2)",
    )
    .bind(&probe)
    .bind(std::process::id() as i32)
    .execute(&raw)
    .await
    .expect("seed a live-pid service row the sweeper must leave alone in a dry run");

    services::execute(parse(&["cleanup", "--dry-run"]), &ctx(&app, true))
        .await
        .expect("json dry-run cleanup");
    services::execute(parse(&["cleanup", "--dry-run"]), &ctx(&app, false))
        .await
        .expect("text dry-run cleanup");

    assert!(
        service_row_exists(&pool, &probe).await,
        "a dry run must never remove a service row"
    );

    sqlx::query("DELETE FROM services WHERE name = $1")
        .bind(&probe)
        .execute(&raw)
        .await
        .unwrap();
}

#[tokio::test]
async fn restart_subcommands_route_through_the_dispatcher() {
    let (_pool, app) = app().await;

    services::execute(parse(&["restart", "--agents"]), &ctx(&app, true))
        .await
        .expect("restart --agents");
    services::execute(parse(&["restart", "--mcp"]), &ctx(&app, true))
        .await
        .expect("restart --mcp");
    services::execute(parse(&["restart", "--failed"]), &ctx(&app, true))
        .await
        .expect("restart --failed");
}

#[tokio::test]
async fn stopping_a_named_agent_that_is_not_running_is_reported_not_silently_ignored() {
    let (pool, app) = app().await;

    let err = services::execute(parse(&["stop", "agent", AGENT_PROBE]), &ctx(&app, true))
        .await
        .expect_err("a name with no running service must be reported, not silently accepted");
    assert!(
        err.to_string().contains(AGENT_PROBE),
        "the error must name the agent asked for, got {err}"
    );

    // The MCP arm does NOT mirror the agent arm: it filters the running set by
    // name, so an unknown name selects nothing and succeeds. Pinned because the
    // asymmetry is load-bearing for scripts that stop agents and servers in one
    // pass and only expect the agent form to fail.
    services::execute(parse(&["stop", "mcp", MCP_PROBE]), &ctx(&app, true))
        .await
        .expect("the MCP arm treats an unmatched name as an empty selection");

    assert!(
        !service_row_exists(&pool, AGENT_PROBE).await
            && !service_row_exists(&pool, MCP_PROBE).await,
        "a refused or empty stop must not create service rows"
    );
}
