//! `plugins mcp status`, `list` and `validate` against a services config that
//! actually declares MCP servers.
//!
//! The shared fixture bootstrap has an empty `mcp_servers` map, so every one of
//! these commands returns before its per-server body runs. This suite boots a
//! config declaring an enabled and a disabled server, both pointed at ports
//! nothing is listening on: the per-server loops run, and every connection
//! attempt lands on the unreachable arm rather than the happy path.
//!
//! `plugins mcp tools` resolves a CLI session first, so it reaches its own body
//! only since the bootstrap fixture started naming its tempdir something
//! `ProfileName` accepts.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::{Arc, OnceLock};

use clap::Parser;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, free_port_in_range,
    init_services_bootstrap, install_test_signing_key,
};

const ENABLED: &str = "fixture_enabled_server";
const DISABLED: &str = "fixture_disabled_server";

fn services_yaml() -> String {
    // Why: both servers are declared `external`. An `internal` one is looked up
    // as a compiled extension, and the status probe fails outright when no
    // `extensions/<name>/manifest.yaml` exists — which is every checkout of
    // this repository, so the whole suite would fail before reaching a command.
    let port = free_port_in_range(5900..6000).expect("a free port in the mcp range");
    format!(
        "mcp_servers:\n  \
         {ENABLED}:\n    type: external\n    binary: \"\"\n    package: null\n    \
         port: {port}\n    endpoint: http://127.0.0.1:{port}/mcp\n    enabled: true\n    \
         display_in_web: false\n    oauth:\n      required: \
         false\n      scopes: []\n      audience: mcp\n      client_id: null\n  \
         {DISABLED}:\n    type: external\n    binary: \"\"\n    package: null\n    port: 0\n    \
         endpoint: http://127.0.0.1:1/mcp\n    enabled: false\n    display_in_web: false\n    \
         oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: \
         null\n"
    )
}

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| {
        let b = init_services_bootstrap(&services_yaml());
        install_test_signing_key();
        b
    })
}

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: McpCommands,
}

fn parse(args: &[&str]) -> McpCommands {
    Harness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

async fn app() -> (DbPool, Arc<AppContext>) {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url)
        .await
        .expect("the mcp command tests need a reachable test database");
    let app = fixture_app_context(&pool, &b.database_url).expect("fixture app context");
    (pool, app)
}

fn ctx(app: &Arc<AppContext>) -> CommandContext {
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        Arc::clone(app),
    )
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    let (_pool, app) = app().await;
    assert_configured();
    mcp::execute(parse(args), &ctx(&app)).await
}

// Why: `ServicesBootstrap` initialises once per process. If a sibling suite
// wins that race the config below is never installed, every command here runs
// against an empty server map, and the assertions would pass while exercising
// nothing. Fail loudly instead.
fn assert_configured() {
    let config = systemprompt_loader::ConfigLoader::load().expect("services config");
    assert!(
        config.mcp_servers.contains_key(ENABLED) && config.mcp_servers.contains_key(DISABLED),
        "the fixture services config was not the one that booted this process; got servers: {:?}",
        config.mcp_servers.keys().collect::<Vec<_>>()
    );
}

fn message(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

#[tokio::test]
async fn status_walks_every_configured_server_when_nothing_is_running() {
    run(&["status"])
        .await
        .expect("an enabled and a disabled server that are both down still render a status");
}

#[tokio::test]
async fn the_detailed_flag_renders_the_same_servers() {
    run(&["status", "--detailed"])
        .await
        .expect("--detailed renders the configured servers");
}

#[tokio::test]
async fn a_server_filter_selects_one_configured_server() {
    run(&["status", "--server", ENABLED])
        .await
        .expect("filtering to a configured server renders that server");
}

#[tokio::test]
async fn a_filter_matching_no_configured_server_is_not_an_error() {
    run(&["status", "--server", "no-such-mcp-server"])
        .await
        .expect("an unmatched status filter renders an empty table rather than failing");
}

#[tokio::test]
async fn listing_configured_servers_reads_the_same_config() {
    run(&["list"])
        .await
        .expect("list renders the configured servers");
    run(&["list", "--enabled"])
        .await
        .expect("the enabled filter renders the enabled subset");
    run(&["list", "--disabled"])
        .await
        .expect("the disabled filter renders the disabled subset");
    run(&["list", "--enabled", "--disabled"])
        .await
        .expect("both filters together fall back to showing everything");
}

#[tokio::test]
async fn validating_every_server_reports_each_one_unreachable() {
    run(&["validate", "--all", "--timeout", "1"])
        .await
        .expect("validation of unreachable servers reports failures rather than erroring");
}

#[tokio::test]
async fn validating_one_named_server_runs_only_that_check() {
    run(&["validate", ENABLED, "--timeout", "1"])
        .await
        .expect("a named server validates on its own");
}

#[tokio::test]
async fn the_service_alias_selects_the_same_server_as_the_positional_name() {
    run(&["validate", "--service", ENABLED, "--timeout", "1"])
        .await
        .expect("--service is an alias for the positional server name");
}

#[tokio::test]
async fn validating_a_server_that_is_not_configured_names_it() {
    let (_pool, app) = app().await;
    assert_configured();
    let ctx = ctx(&app);

    let err = mcp::execute(parse(&["validate", "ghost-server", "--timeout", "1"]), &ctx)
        .await
        .expect_err("a server absent from the config cannot be validated");

    assert!(
        message(&err).contains("ghost-server"),
        "the refusal should name the server that is not configured, got: {}",
        message(&err)
    );
}

#[tokio::test]
async fn listing_tools_refuses_when_no_server_is_running() {
    let err = run(&["tools", "--timeout", "1"])
        .await
        .expect_err("tools cannot be listed when nothing is running");

    assert!(
        message(&err).to_lowercase().contains("running"),
        "the refusal should say no server is running, got: {}",
        message(&err)
    );
}

#[tokio::test]
async fn listing_tools_for_a_named_server_that_is_not_running_names_it() {
    let err = run(&["tools", "--server", ENABLED, "--timeout", "1"])
        .await
        .expect_err("a configured but stopped server has no tools to list");

    assert!(
        message(&err).contains(ENABLED),
        "the refusal should name the server asked for, got: {}",
        message(&err)
    );
}
