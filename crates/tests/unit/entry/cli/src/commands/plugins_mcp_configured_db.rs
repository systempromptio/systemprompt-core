//! `plugins mcp status`, `list` and `validate` against a services config that
//! actually declares MCP servers.
//!
//! The shared fixture bootstrap has an empty `mcp_servers` map, so every one of
//! these commands returns before its per-server body runs. This suite boots a
//! config declaring an enabled and a disabled server, both pointed at ports
//! nothing is listening on: the per-server loops run, and every connection
//! attempt lands on the unreachable arm rather than the happy path.
//!
//! `plugins mcp tools` resolves a CLI session before anything else, so this
//! suite seeds one for the bootstrap profile. Without it the command dies on
//! "No session for active profile" and never reaches the unreachable-server
//! arm these tests assert on.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::{Arc, OnceLock};

use clap::Parser;
use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, fixture_user_id, free_port_in_range,
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
         {ENABLED}:\n    type: external\n    \
         endpoint: http://127.0.0.1:{port}/mcp\n    enabled: true\n    \
         display_in_web: false\n    oauth:\n      required: \
         false\n      scopes: []\n      audience: mcp\n      client_id: null\n  \
         {DISABLED}:\n    type: external\n    \
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
        seed_cli_session(&b);
        b
    })
}

fn seed_cli_session(b: &TestBootstrap) {
    let profile_dir = b
        .profile_path
        .parent()
        .expect("the bootstrap profile.yaml has a parent directory");
    let profile_name_str = profile_dir
        .file_name()
        .and_then(|n| n.to_str())
        .expect("the bootstrap profile directory has a usable name")
        .to_owned();
    let profile_name = ProfileName::try_new(profile_name_str.as_str())
        .expect("the bootstrap profile directory name is a valid ProfileName");

    let session = CliSession::builder(
        SessionBinding::new(profile_name, "https://issuer.test".to_owned()),
        SessionToken::new("fixture-session-token"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            fixture_user_id(),
            Email::try_new("fixture@example.com").expect("fixture email"),
            UserType::Admin,
        ),
    )
    .with_profile_path(&b.profile_path)
    .build();

    // Why: the store persists between runs, so a stale `active_profile_name`
    // from an earlier fixture tempdir would not match this one and the session
    // would be rejected before the command body runs. Claim both.
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store =
        SessionStore::load_or_create(&sessions_dir).expect("load the CLI session store");
    store.upsert_session(&SessionKey::Local, session);
    store.set_active_with_profile(&SessionKey::Local, profile_name_str.as_str());
    store
        .save(&sessions_dir)
        .expect("persist the CLI session store");
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

const VALIDATE_EXTERNAL_HELPER: &str =
    "commands::plugins_mcp_configured_db::validate_running_external_helper";

#[tokio::test]
#[ignore = "re-executed by running_external_validation_reports_its_configuration_error"]
async fn validate_running_external_helper() {
    use systemprompt_database::CreateServiceInput;
    use systemprompt_test_fixtures::DisposableDb;

    boot();
    assert_configured();
    let database = DisposableDb::installed("cli_mcp_validate_external")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let app = fixture_app_context(&pool, database.url()).expect("fixture app context");
    app.service_repository()
        .create_service(CreateServiceInput {
            name: ENABLED,
            module_name: "mcp",
            status: "running",
            port: 1,
            binary_mtime: None,
        })
        .await
        .expect("register the external MCP service as running");

    let context = ctx(&app);
    println!("BEGIN_VALIDATE_EXTERNAL");
    mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &context)
        .await
        .expect("validation reports a structured configuration failure");
    println!("END_VALIDATE_EXTERNAL");

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
    "commands::plugins_mcp_configured_db::validate_closed_database_helper";
const VALIDATE_STOPPED_HELPER: &str =
    "commands::plugins_mcp_configured_db::validate_stopped_outputs_helper";

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

fn marked_json(stdout: &str, begin: &str, end: &str) -> serde_json::Value {
    let body = stdout
        .split_once(begin)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(body, _)| body.trim())
        .unwrap_or_else(|| panic!("missing {begin}/{end}: {stdout}"));
    serde_json::from_str(body).expect("validation JSON artifact")
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
    let database = DisposableDb::installed("cli_mcp_validate_closed_database")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let app = fixture_app_context(&pool, database.url()).expect("fixture app context");
    pool.pool_arc()
        .expect("initialized SQLx pool")
        .close()
        .await;
    println!("BEGIN_VALIDATE_CLOSED_DATABASE");
    mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &ctx(&app))
        .await
        .expect("database lookup failure renders structured validation output");
    println!("END_VALIDATE_CLOSED_DATABASE");
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
    let database = DisposableDb::installed("cli_mcp_validate_stopped_outputs")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let app = fixture_app_context(&pool, database.url()).expect("fixture app context");
    let context = ctx(&app);
    println!("BEGIN_VALIDATE_NAMED_STOPPED");
    mcp::execute(parse(&["validate", ENABLED, "--timeout", "1"]), &context)
        .await
        .expect("named stopped validation renders structured output");
    println!("END_VALIDATE_NAMED_STOPPED");
    println!("BEGIN_VALIDATE_BATCH_STOPPED");
    mcp::execute(parse(&["validate", "--all", "--timeout", "1"]), &context)
        .await
        .expect("batch stopped validation renders structured output");
    println!("END_VALIDATE_BATCH_STOPPED");
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
