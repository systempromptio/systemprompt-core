#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::Command;

use clap::Parser;
use systemprompt_cli::EnvOverrides;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_test_fixtures::{DisposableDb, bounded_output, init_services_bootstrap};

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
    let extension = boot.system_path.join("extensions").join(SERVER);
    std::fs::create_dir_all(&extension).expect("create owned extension directory");
    std::fs::write(
        extension.join("manifest.yaml"),
        format!(
            "extension:\n  type: mcp\n  name: {SERVER}\n  binary: {BINARY}\n  description: status fixture\n  enabled: true\n"
        ),
    )
    .expect("write internal extension manifest");

    let release_binary = boot
        .bin_path
        .parent()
        .expect("the bootstrap bin path has a parent")
        .join("release")
        .join(BINARY);
    std::fs::create_dir_all(release_binary.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&boot.bin_path).unwrap();
    std::fs::write(&release_binary, "owned fixture binary\n").unwrap();

    let database = DisposableDb::with_schema("cli_mcp_internal_status").await;
    let pool = database.test_pool().await;
    let app = crate::boot_ctx::app_on_database(&boot, database.url()).await;
    let context = crate::boot_ctx::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    println!("BEGIN_COMPACT_STATUS");
    crate::boot_ctx::rendered(
        &context,
        mcp::execute(parse(&["status", "--server", SERVER]), &context).await,
    )
    .expect("compact internal status");
    println!("END_COMPACT_STATUS");
    println!("BEGIN_DETAILED_STATUS");
    crate::boot_ctx::rendered(
        &context,
        mcp::execute(
            parse(&["status", "--server", SERVER, "--detailed"]),
            &context,
        )
        .await,
    )
    .expect("detailed internal status");
    println!("END_DETAILED_STATUS");

    drop(context);
    pool.write_pool().close().await;
    drop(pool);
    database.drop_now().await;
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
    let output = bounded_output(command, "mcp internal status lifecycle");
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
