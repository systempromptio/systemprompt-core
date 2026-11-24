//! `plugins mcp status`, `list` and `validate` against a services config that
//! actually declares MCP servers.
//!
//! The shared fixture bootstrap has an empty `mcp_servers` map, so every one of
//! these commands returns before its per-server body runs. This suite boots a
//! config declaring an enabled and a disabled server, both pointed at ports
//! nothing is listening on: the per-server loops run, and every connection
//! attempt lands on the unreachable arm rather than the happy path.
//!
//! `plugins mcp tools` resolves the caller's session only when a queried
//! server requires OAuth; neither fixture server does and neither is
//! running, so no session is seeded.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::{Arc, OnceLock};
use systemprompt_test_fixtures::AppContextSeeds as _;

use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_mcp_entry::cli as mcp;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ReservedPort, TestBootstrap, init_services_bootstrap, seed_fixture_system_admin, test_db_pool,
};

use crate::common::{json_cli, marked_json, member, parse, rendered, with_app};

const ENABLED: &str = "fixture_enabled_server";
const DISABLED: &str = "fixture_disabled_server";

fn services_yaml() -> String {
    // Why: both servers are declared `external`. An `internal` one is looked up
    // as a compiled extension, and the status probe fails outright when no
    // `extensions/<name>/manifest.yaml` exists — which is every checkout of
    // this repository, so the whole suite would fail before reaching a command.
    let port = ReservedPort::in_range(5900..6000)
        .expect("a free port in the mcp range")
        .release_for_child();
    format!(
        "mcp_servers:\n  \
         {ENABLED}:\n    server_type: external\n    \
         endpoint: http://127.0.0.1:{port}/mcp\n    enabled: true\n    \
         display_in_web: false\n    oauth:\n      required: \
         false\n      scopes: []\n      audience: mcp\n      client_id: null\n  \
         {DISABLED}:\n    server_type: external\n    \
         endpoint: http://127.0.0.1:1/mcp\n    enabled: false\n    display_in_web: false\n    \
         oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: \
         null\n"
    )
}

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_services_bootstrap(&services_yaml()))
}

async fn app_over(database_url: &str) -> Arc<AppContext> {
    systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        boot(),
        systemprompt_test_fixtures::fixture_config(database_url),
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await
    .expect("fixture AppContext over the services bootstrap")
}

async fn app() -> (DbPool, Arc<AppContext>) {
    boot();
    let pool = test_db_pool().await;
    seed_fixture_system_admin(&pool)
        .await
        .expect("seed the configured system admin");
    let app = app_over(&boot().database_url).await;
    (pool, app)
}

fn ctx(app: &Arc<AppContext>) -> CommandContext {
    with_app(json_cli(), EnvOverrides::default(), Arc::clone(app))
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    let (_pool, app) = app().await;
    assert_configured();
    let context = ctx(&app);
    mcp::execute(parse(args), &member(&context)).await.map(drop)
}

fn assert_configured() {
    let config = boot().boot.load_services_config().expect("services config");
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
async fn the_service_alias_selects_the_same_server_as_the_positional_name() {
    let err = run(&["validate", "--service", ENABLED, "--timeout", "1"])
        .await
        .expect_err("the stopped server resolves but does not validate");
    assert!(
        message(&err).contains("MCP server validation failed"),
        "--service must resolve the configured server, got: {}",
        message(&err)
    );
}

#[tokio::test]
async fn validating_a_server_that_is_not_configured_names_it() {
    let (_pool, app) = app().await;
    assert_configured();
    let context = ctx(&app);

    let err = mcp::execute(
        parse(&["validate", "ghost-server", "--timeout", "1"]),
        &member(&context),
    )
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

const VALIDATE_EXTERNAL_HELPER: &str =
    "plugins_mcp_configured_db::validate_running_external_helper";

#[tokio::test]
#[ignore = "re-executed by running_external_validation_reports_its_configuration_error"]
async fn validate_running_external_helper() {
    use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceStatus};
    use systemprompt_identifiers::ServiceName;
    use systemprompt_test_fixtures::DisposableDb;

    boot();
    assert_configured();
    let database = DisposableDb::with_schema("cli_mcp_validate_external").await;
    let pool = database.test_pool().await;
    let app = app_over(database.url()).await;
    app.service_repository()
        .create_service(CreateServiceInput {
            name: &ServiceName::new(ENABLED),
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Running,
            port: 1,
            binary_mtime: None,
        })
        .await
        .expect("register the external MCP service as running");

    let context = ctx(&app);
    let host = member(&context);
    println!("BEGIN_VALIDATE_EXTERNAL");
    rendered(
        &host,
        mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &host).await,
    )
    .expect_err("an invalid server renders its report and exits non-zero");
    println!("END_VALIDATE_EXTERNAL");

    drop(host);
    drop(context);
    drop(app);
    drop(pool);
    database.drop_now().await;
}

#[test]
fn running_external_validation_reports_its_configuration_error() {
    let stdout = capture_validate_helper(VALIDATE_EXTERNAL_HELPER);
    let artifact = marked_json(&stdout, "BEGIN_VALIDATE_EXTERNAL", "END_VALIDATE_EXTERNAL");
    assert_eq!(artifact["title"], format!("MCP Validation: {ENABLED}"));
    let sections = artifact["sections"]
        .as_array()
        .expect("validation card sections");
    let content = |heading: &str| {
        &sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
    };
    assert_eq!(
        content("summary"),
        &serde_json::json!({
            "total": 1,
            "valid": 0,
            "invalid": 1,
            "healthy": 0,
            "unhealthy": 1
        }),
        "{artifact}"
    );
    let results = content("results")
        .as_array()
        .expect("validation result rows");
    assert_eq!(results.len(), 1, "{artifact}");
    assert_eq!(
        results[0],
        serde_json::json!({
            "server": ENABLED,
            "valid": false,
            "health_status": "unknown",
            "validation_type": "config_error",
            "latency_ms": 0,
            "issues": ["Server declares no local port; external servers are validated at their endpoint"],
            "message": format!("MCP server '{ENABLED}' has no local port")
        }),
        "{artifact}"
    );
}
const VALIDATE_CLOSED_DATABASE_HELPER: &str =
    "plugins_mcp_configured_db::validate_closed_database_helper";
const VALIDATE_STOPPED_HELPER: &str = "plugins_mcp_configured_db::validate_stopped_outputs_helper";

struct ValidateChild(std::process::Child);

impl Drop for ValidateChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn capture_validate_helper(name: &str) -> String {
    use std::io::{Read, Seek, SeekFrom};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let stdout = tempfile::tempfile().expect("owned stdout capture");
    let stderr = tempfile::tempfile().expect("owned stderr capture");
    let mut child = ValidateChild(
        Command::new(std::env::current_exe().expect("unit-test binary path"))
            .args(["--exact", name, "--ignored", "--nocapture"])
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                stdout.try_clone().expect("clone stdout capture"),
            ))
            .stderr(Stdio::from(
                stderr.try_clone().expect("clone stderr capture"),
            ))
            .spawn()
            .expect("spawn isolated validation helper"),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll validation helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.0.kill();
            let _ = child.0.wait();
            panic!("validation helper exceeded thirty seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let read_capture = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read capture");
        text
    };
    let stdout = read_capture(stdout);
    let stderr = read_capture(stderr);
    assert!(
        status.success(),
        "validation helper failed; stdout={stdout:?}; stderr={stderr:?}"
    );
    stdout
}

fn validation_content<'a>(artifact: &'a serde_json::Value, heading: &str) -> &'a serde_json::Value {
    &artifact["sections"]
        .as_array()
        .expect("validation sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
}

#[tokio::test]
#[ignore = "re-executed by validation_reports_database_lookup_failure"]
async fn validate_closed_database_helper() {
    use systemprompt_test_fixtures::DisposableDb;

    boot();
    assert_configured();
    let database = DisposableDb::with_schema("cli_mcp_validate_closed_database").await;
    let app = app_over(database.url()).await;
    app.db_pool().pool().close().await;
    app.db_pool().write_pool().close().await;
    let context = ctx(&app);
    let host = member(&context);
    println!("BEGIN_VALIDATE_CLOSED_DATABASE");
    rendered(
        &host,
        mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &host).await,
    )
    .expect_err("database lookup failure renders its report and exits non-zero");
    println!("END_VALIDATE_CLOSED_DATABASE");
    drop(host);
    drop(context);
    drop(app);
    database.drop_now().await;
}

#[test]
fn validation_reports_database_lookup_failure() {
    let stdout = capture_validate_helper(VALIDATE_CLOSED_DATABASE_HELPER);
    let artifact = marked_json(
        &stdout,
        "BEGIN_VALIDATE_CLOSED_DATABASE",
        "END_VALIDATE_CLOSED_DATABASE",
    );
    assert_eq!(artifact["title"], format!("MCP Validation: {ENABLED}"));
    assert_eq!(
        validation_content(&artifact, "summary"),
        &serde_json::json!({
            "total": 1, "valid": 0, "invalid": 1, "healthy": 0, "unhealthy": 1
        })
    );
    let results = validation_content(&artifact, "results")
        .as_array()
        .expect("validation result rows");
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result["server"], ENABLED);
    assert_eq!(result["valid"], false);
    assert_eq!(result["health_status"], "unknown");
    assert_eq!(result["validation_type"], "database_error");
    assert_eq!(result["latency_ms"], 0);
    assert!(
        result["issues"]
            .to_string()
            .contains("Failed to check service status")
    );
}

#[tokio::test]
#[ignore = "re-executed by validation_reports_exact_named_and_batch_stopped_results"]
async fn validate_stopped_outputs_helper() {
    use systemprompt_test_fixtures::DisposableDb;

    boot();
    assert_configured();
    let database = DisposableDb::with_schema("cli_mcp_validate_stopped_outputs").await;
    let pool = database.test_pool().await;
    let app = app_over(database.url()).await;
    let context = ctx(&app);
    let host = member(&context);
    println!("BEGIN_VALIDATE_NAMED_STOPPED");
    rendered(
        &host,
        mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &host).await,
    )
    .expect_err("a stopped server renders its report and exits non-zero");
    println!("END_VALIDATE_NAMED_STOPPED");
    println!("BEGIN_VALIDATE_BATCH_STOPPED");
    rendered(
        &host,
        mcp::execute(parse(&["validate", "--all", "--timeout", "1"]), &host).await,
    )
    .expect_err("a stopped batch renders its report and exits non-zero");
    println!("END_VALIDATE_BATCH_STOPPED");
    drop(host);
    drop(context);
    drop(app);
    drop(pool);
    database.drop_now().await;
}

#[test]
fn validation_reports_exact_named_and_batch_stopped_results() {
    let stdout = capture_validate_helper(VALIDATE_STOPPED_HELPER);
    let named = marked_json(
        &stdout,
        "BEGIN_VALIDATE_NAMED_STOPPED",
        "END_VALIDATE_NAMED_STOPPED",
    );
    assert_eq!(named["title"], format!("MCP Validation: {ENABLED}"));
    assert_eq!(
        validation_content(&named, "summary"),
        &serde_json::json!({
            "total": 1, "valid": 0, "invalid": 1, "healthy": 0, "unhealthy": 1
        })
    );
    let named_results = validation_content(&named, "results").as_array().unwrap();
    assert_eq!(named_results.len(), 1);
    assert_eq!(named_results[0]["server"], ENABLED);
    assert_eq!(named_results[0]["health_status"], "stopped");
    assert_eq!(named_results[0]["validation_type"], "not_running");

    let batch = marked_json(
        &stdout,
        "BEGIN_VALIDATE_BATCH_STOPPED",
        "END_VALIDATE_BATCH_STOPPED",
    );
    assert_eq!(batch["title"], "MCP Batch Validation Results");
    assert_eq!(
        validation_content(&batch, "summary"),
        &serde_json::json!({
            "total": 2, "valid": 0, "invalid": 2, "healthy": 0, "unhealthy": 2
        })
    );
    let results = validation_content(&batch, "results").as_array().unwrap();
    assert_eq!(results.len(), 2);
    let by_name = results
        .iter()
        .map(|row| (row["server"].as_str().unwrap(), row))
        .collect::<std::collections::HashMap<_, _>>();
    for server in [ENABLED, DISABLED] {
        assert_eq!(by_name[server]["health_status"], "stopped");
        assert_eq!(by_name[server]["validation_type"], "not_running");
        assert_eq!(
            by_name[server]["issues"],
            serde_json::json!(["Service is not currently running"])
        );
    }
}
