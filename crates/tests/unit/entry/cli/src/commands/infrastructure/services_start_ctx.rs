//! `infra services start` on the paths that do not spawn anything.
//!
//! `--agents` and `--mcp` without `--api` compute a startup plan whose only
//! outcome is the "standalone start not supported" notice: the renderer, the
//! pre-flight phase and the plan computation all run, but no process is
//! spawned and no port is bound. `start agent <name>` / `start mcp <name>`
//! resolve the name first, so an unregistered one fails before any spawn.
//!
//! `--api` and `--all` are never driven here — those genuinely start a server.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::infrastructure::services::start::{
    ServiceFlags, ServiceTarget, ServiceTargetFlags,
};
use systemprompt_cli::infrastructure::services::{self, ServicesCommands, load_service_configs};
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

async fn applied_migration_count(pool: &DbPool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM extension_migrations")
        .fetch_one(pool.pool_arc().unwrap().as_ref())
        .await
        .expect("count applied migrations")
}

// Scoped by name: sibling suites write to the same `services` table, so a
// whole-table comparison would race. Every probe name below is one no manifest
// declares, so a row bearing it can only have come from the command under test.
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
async fn starting_agents_standalone_reports_the_notice_without_spawning() {
    let (pool, app) = app().await;

    services::execute(
        parse(&["start", "--agents", "--skip-migrate"]),
        &ctx(&app, false),
    )
    .await
    .expect("a standalone agent start is a notice, not a failure");

    assert!(
        !service_row_exists(&pool, "api").await,
        "the standalone notice path must not register the API service"
    );
}

#[tokio::test]
async fn starting_mcp_standalone_reports_the_notice_without_spawning() {
    let (pool, app) = app().await;

    services::execute(
        parse(&["start", "--mcp", "--skip-migrate"]),
        &ctx(&app, true),
    )
    .await
    .expect("a standalone MCP start is a notice, not a failure");

    assert!(
        !service_row_exists(&pool, "api").await,
        "the standalone notice path must not register the API service"
    );
}

#[tokio::test]
async fn both_standalone_notices_can_fire_in_one_invocation() {
    let (pool, app) = app().await;

    services::execute(
        parse(&["start", "--agents", "--mcp", "--skip-migrate"]),
        &ctx(&app, false),
    )
    .await
    .expect("both notices");

    assert!(!service_row_exists(&pool, "api").await);
}

#[tokio::test]
async fn a_start_without_skip_migrate_runs_the_migration_phase_idempotently() {
    // The startup plan installs a schema, so it gets a database created for
    // this run: an install onto whatever an earlier run left behind proves
    // nothing about the phase under test.
    ensure_test_bootstrap();
    install_test_signing_key();
    let disp = DisposableDb::installed("cov_cli_startmig")
        .await
        .expect("a freshly installed disposable database");
    let pool = disp.pool().await.expect("pool on the disposable database");
    let app = fixture_app_context(&pool, disp.url()).expect("fixture app context");

    let applied_before = applied_migration_count(&pool).await;

    // Without --skip-migrate the plan includes the Database phase, which runs
    // `db migrate` — a no-op against an already-migrated database.
    services::execute(parse(&["start", "--agents"]), &ctx(&app, false))
        .await
        .expect("the migration phase runs then the standalone notice fires");

    let applied_after = applied_migration_count(&pool).await;
    drop(pool);
    drop(app);
    disp.drop_now().await;
    assert_eq!(
        applied_after, applied_before,
        "the startup migration phase must be idempotent"
    );
}

#[tokio::test]
async fn starting_a_named_agent_that_is_not_registered_fails_before_spawning() {
    let (pool, app) = app().await;

    let err = services::execute(parse(&["start", "agent", AGENT_PROBE]), &ctx(&app, false))
        .await
        .expect_err("an unregistered agent cannot be started");
    assert!(
        err.to_string().contains(AGENT_PROBE),
        "the failure must name the agent asked for, got {err}"
    );

    assert!(
        !service_row_exists(&pool, AGENT_PROBE).await,
        "a name that does not resolve must not leave a service row behind"
    );
}

#[tokio::test]
async fn starting_a_named_mcp_server_that_is_not_registered_selects_nothing() {
    let (pool, app) = app().await;

    // The MCP orchestrator filters by name, so an unmatched name is an empty
    // selection rather than an error — the same asymmetry `stop` has.
    services::execute(parse(&["start", "mcp", MCP_PROBE]), &ctx(&app, false))
        .await
        .expect("an unmatched MCP name selects nothing");

    assert!(
        !service_row_exists(&pool, MCP_PROBE).await,
        "an unmatched name must not register a service row"
    );
}

#[tokio::test]
async fn restart_with_no_target_and_no_flag_is_refused_with_guidance() {
    let (_pool, app) = app().await;

    let err = services::execute(parse(&["restart"]), &ctx(&app, false))
        .await
        .expect_err("a restart with nothing selected cannot proceed");
    let message = err.to_string();
    assert!(
        message.contains("--failed") && message.contains("--agents"),
        "the refusal must list the flags that would select a target, got {message}"
    );
}

#[test]
fn service_flags_translate_into_a_target_set() {
    let all = ServiceTarget::from_flags(ServiceFlags {
        all: true,
        targets: ServiceTargetFlags {
            api: false,
            agents: false,
            mcp: false,
        },
    });
    assert!(
        all.api && all.agents && all.mcp,
        "--all must select every service kind"
    );

    let only_mcp = ServiceTarget::from_flags(ServiceFlags {
        all: false,
        targets: ServiceTargetFlags {
            api: false,
            agents: false,
            mcp: true,
        },
    });
    assert!(
        !only_mcp.api && !only_mcp.agents && only_mcp.mcp,
        "an explicit single flag must not widen the selection"
    );

    let none = ServiceTarget::from_flags(ServiceFlags {
        all: false,
        targets: ServiceTargetFlags {
            api: false,
            agents: false,
            mcp: false,
        },
    });
    assert!(
        none.api && none.agents && none.mcp,
        "no flags at all is the same as --all: a bare `services start` starts everything"
    );
}

#[test]
fn the_service_manifest_loads_from_the_bootstrap_profile() {
    ensure_test_bootstrap();

    let configs = load_service_configs().expect("the bootstrap profile has a readable manifest");
    assert!(
        configs.iter().all(|c| !c.name.is_empty()),
        "every manifest entry must carry a service name"
    );
}
