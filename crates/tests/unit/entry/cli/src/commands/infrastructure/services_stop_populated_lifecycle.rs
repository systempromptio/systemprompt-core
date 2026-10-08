//! Public grouped service stop selects agent and MCP rows and persists their
//! state.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use serde_json::Value;
use systemprompt_cli::infrastructure::services::{self, ServicesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, install_test_signing_key, test_app_context,
};

const HELPER: &str =
    "commands::infrastructure::services_stop_populated_lifecycle::populated_stop_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: ServicesCommands,
}

fn parse(args: &[&str]) -> ServicesCommands {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
        .expect("parse services command")
        .command
}

fn section<'a>(card: &'a Value, heading: &str) -> &'a Value {
    card["sections"]
        .as_array()
        .expect("stop card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .map(|section| &section["content"])
        .unwrap_or_else(|| panic!("missing {heading}: {card}"))
}

#[tokio::test]
#[ignore = "re-executed by grouped_stop_reports_exact_counts_and_preserves_unselected_service"]
async fn populated_stop_helper() {
    ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::with_schema("cli_grouped_stop").await;
    let database_info_path =
        std::env::var_os("GROUPED_STOP_DATABASE_INFO").expect("private database info channel");
    std::fs::write(database_info_path, database.url()).expect("write private database info");
    let pool = database.test_pool().await;
    let app = test_app_context(&pool, database.url());
    let repository = app.service_repository().clone();
    for (name, module, port) in [
        ("owned-stop-agent", ServiceModule::Agent, 9311),
        ("owned-stop-mcp", ServiceModule::Mcp, 9312),
    ] {
        repository
            .create_service(CreateServiceInput {
                name: &ServiceName::new(name),
                module_name: module,
                status: ServiceStatus::Running,
                port,
                binary_mtime: None,
            })
            .await
            .expect("seed scoped running service");
    }
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, status, port) VALUES ($1, \
         'owned-stop-control', 'api-control', 'running', 9313)",
    )
    .bind(repository.instance_id().as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect("seed a row of a module the grouped stop does not select");
    let ctx = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    println!("BEGIN_STOP_ARTIFACT");
    services::execute(parse(&["stop", "--agents", "--mcp"]), &ctx)
        .await
        .expect("stop selected populated service groups");
    println!("END_STOP_ARTIFACT");

    let raw = pool.pool();
    let states: Vec<(String, String)> =
        sqlx::query_as("SELECT name, status FROM services WHERE name = ANY($1) ORDER BY name")
            .bind(vec![
                "owned-stop-agent",
                "owned-stop-mcp",
                "owned-stop-control",
            ])
            .fetch_all(raw.as_ref())
            .await
            .expect("read durable service states");
    println!(
        "STOP_STATES={}",
        serde_json::to_string(&states).expect("serialize service states")
    );

    drop(raw);
    drop(ctx);
    drop(repository);
    pool.write_pool().close().await;
    drop(pool);
    database.drop_now().await;
}

struct OwnedChild(Option<Child>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn captured_helper() -> String {
    let stdout = tempfile::tempfile().expect("owned grouped-stop stdout");
    let stderr = tempfile::tempfile().expect("owned grouped-stop stderr");
    let database_info = tempfile::NamedTempFile::new().expect("owned database info channel");
    let child = Command::new(std::env::current_exe().expect("CLI unit test binary"))
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .env("GROUPED_STOP_DATABASE_INFO", database_info.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().expect("clone stdout")))
        .stderr(Stdio::from(stderr.try_clone().expect("clone stderr")))
        .spawn()
        .expect("spawn grouped-stop helper");
    let mut child = OwnedChild(Some(child));
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child
            .0
            .as_mut()
            .expect("child present")
            .try_wait()
            .expect("poll child")
        {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "grouped-stop helper exceeded sixty seconds"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    child.0.take();
    let read = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut value = String::new();
        file.read_to_string(&mut value).expect("read capture");
        value
    };
    let stdout = read(stdout);
    let stderr = read(stderr);
    let database_url = std::fs::read_to_string(database_info.path()).unwrap_or_default();
    let database_password = url::Url::parse(&database_url)
        .ok()
        .and_then(|url| url.password().map(str::to_owned))
        .unwrap_or_default();
    let sanitize = |value: &str| {
        [database_url.as_str(), database_password.as_str()]
            .iter()
            .fold(value.to_owned(), |safe, secret| {
                if secret.is_empty() {
                    safe
                } else {
                    safe.replace(secret, "<REDACTED>")
                }
            })
    };
    assert!(
        status.success(),
        "grouped-stop helper failed; stdout={}; stderr={}",
        sanitize(&stdout),
        sanitize(&stderr)
    );
    sanitize(&stdout)
}

#[test]
fn grouped_stop_reports_exact_counts_and_preserves_unselected_service() {
    let stdout = captured_helper();
    let artifact = stdout
        .split_once("BEGIN_STOP_ARTIFACT")
        .and_then(|(_, tail)| tail.split_once("END_STOP_ARTIFACT"))
        .map(|(json, _)| json.trim())
        .unwrap_or_else(|| panic!("missing stop artifact markers: {stdout}"));
    let card: Value = serde_json::from_str(artifact)
        .unwrap_or_else(|error| panic!("stop artifact is not JSON: {error}: {artifact}"));
    assert_eq!(card["title"], "Stop Services");
    assert_eq!(section(&card, "api_stopped"), false);
    assert_eq!(section(&card, "agents_stopped"), 1);
    assert_eq!(section(&card, "mcp_stopped"), 1);
    assert_eq!(section(&card, "message"), "All requested services stopped");

    let states = stdout
        .lines()
        .find_map(|line| line.strip_prefix("STOP_STATES="))
        .expect("durable state marker");
    assert_eq!(
        serde_json::from_str::<Value>(states).expect("durable states JSON"),
        serde_json::json!([
            ["owned-stop-agent", "stopped"],
            ["owned-stop-control", "running"],
            ["owned-stop-mcp", "stopped"]
        ])
    );
}
