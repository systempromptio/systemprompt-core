#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    DisposableDb, fixture_app_context_with, init_services_bootstrap, install_test_signing_key,
};

const SERVER: &str = "cov_internal_status";
const BINARY: &str = "cov-internal-mcp";
const HELPER: &str = "commands::mcp_internal_status_lifecycle::internal_status_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: McpCommands,
}

fn parse(args: &[&str]) -> McpCommands {
    Harness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .expect("parse MCP status command")
        .command
}

fn services_yaml() -> String {
    format!(
        "mcp_servers:\n  {SERVER}:\n    server_type: internal\n    binary: {BINARY}\n    package: null\n    port: 5999\n    enabled: true\n    display_in_web: true\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: null\n"
    )
}

#[tokio::test]
#[ignore = "re-executed by internal_status_reports_stopped_state_and_owned_binary_availability"]
async fn internal_status_helper() {
    let boot = init_services_bootstrap(&services_yaml());
    install_test_signing_key();
    let extension = boot.system_path.join("extensions").join(SERVER);
    std::fs::create_dir_all(&extension).expect("create owned extension directory");
    std::fs::write(
        extension.join("manifest.yaml"),
        format!(
            "extension:\n  type: mcp\n  name: {SERVER}\n  binary: {BINARY}\n  description: status fixture\n  enabled: true\n"
        ),
    )
    .expect("write internal extension manifest");

    let selected_bin = boot._tmp.path().join("build/current");
    let release_binary = boot._tmp.path().join("build/release").join(BINARY);
    std::fs::create_dir_all(release_binary.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&selected_bin).unwrap();
    std::fs::write(&release_binary, "owned fixture binary\n").unwrap();

    let database = DisposableDb::installed("cli_mcp_internal_status")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let paths = PathsConfig {
        system: boot.system_path.to_string_lossy().into_owned(),
        services: boot.services_path.to_string_lossy().into_owned(),
        bin: selected_bin.to_string_lossy().into_owned(),
        web_path: Some(boot.system_path.join("web").to_string_lossy().into_owned()),
        storage: Some(boot.storage_path.to_string_lossy().into_owned()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(&pool, database.url(), paths, Arc::new(AllowAllFilter))
        .expect("fixture app context with owned build paths");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    println!("BEGIN_COMPACT_STATUS");
    mcp::execute(parse(&["status", "--server", SERVER]), &context)
        .await
        .expect("compact internal status");
    println!("END_COMPACT_STATUS");
    println!("BEGIN_DETAILED_STATUS");
    mcp::execute(
        parse(&["status", "--server", SERVER, "--detailed"]),
        &context,
    )
    .await
    .expect("detailed internal status");
    println!("END_DETAILED_STATUS");

    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("status stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("status stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn status helper");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll status helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap status helper");
            panic!(
                "status helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn marked_json(stdout: &str, start: &str, end: &str) -> serde_json::Value {
    let json = stdout
        .split_once(start)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing {start}/{end} markers in {stdout}"));
    serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid status JSON: {error}: {json}"))
}

#[test]
fn internal_status_reports_stopped_state_and_owned_binary_availability() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "status helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("status output UTF-8");
    let compact = marked_json(&stdout, "BEGIN_COMPACT_STATUS", "END_COMPACT_STATUS");
    let detailed = marked_json(&stdout, "BEGIN_DETAILED_STATUS", "END_DETAILED_STATUS");
    for artifact in [&compact, &detailed] {
        let rows = artifact["items"].as_array().expect("status rows");
        assert_eq!(rows.len(), 1, "{artifact}");
        assert_eq!(rows[0]["name"], SERVER, "{artifact}");
        assert_eq!(rows[0]["server_type"], "internal", "{artifact}");
        assert_eq!(rows[0]["enabled"], true, "{artifact}");
        assert_eq!(rows[0]["running"], false, "{artifact}");
        assert_eq!(rows[0]["health"], "unhealthy", "{artifact}");
        assert_eq!(rows[0]["port"], 5999, "{artifact}");
        assert_eq!(rows[0]["binary"], BINARY, "{artifact}");
        assert!(rows[0]["debug_binary"].is_null(), "{artifact}");
    }
    assert_eq!(compact["items"][0]["release_binary"], "exists", "{compact}");
    assert!(
        detailed["items"][0]["release_binary"]
            .as_str()
            .is_some_and(|path| path.ends_with(&format!("/release/{BINARY}"))),
        "{detailed}"
    );
}
