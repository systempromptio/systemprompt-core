//! Profile-backed tests for the `admin agents` config commands.
//!
//! `list`, `show`, `validate`, `create`, and `edit` all read (and write) the
//! services config the bootstrapped profile points at, so the fixture tree is
//! seeded per test and the create/edit paths asserted against the files they
//! produce.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::{AgentsCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

fn agent_yaml(name: &str, port: u16, display: &str, enabled: bool) -> String {
    format!(
        r#"agents:
  {name}:
    name: {name}
    port: {port}
    endpoint: /api/v1/agents/{name}/
    enabled: {enabled}
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {name}
      displayName: {display}
      description: Fixture agent for coverage
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {{}}
    oauth:
      required: false
      scopes: []
      audience: a2a
"#
    )
}

#[tokio::test]
async fn restarting_failed_agent_reports_failure_and_preserves_failed_state() {
    use std::sync::Arc;
    use systemprompt_agent::services::agent_orchestration::port_service::{
        find_process_using_port, is_agent_process,
    };
    use systemprompt_cli::infrastructure::services::restart;
    use systemprompt_test_fixtures::{
        DisposableDb, fixture_app_context_with, install_test_signing_key,
    };

    let root = seed_agents();
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an available owned agent port");
    let port = listener.local_addr().unwrap().port();
    assert_eq!(
        find_process_using_port(port).unwrap(),
        Some(std::process::id()),
        "the occupied port must belong to this test process"
    );
    assert!(
        !is_agent_process(std::process::id()).unwrap(),
        "the production port cleanup classifier must reject the unit-test process"
    );
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", port, "Coverage Lister", true),
    )
    .unwrap();
    systemprompt_test_fixtures::refresh_services_config();

    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::installed("cli_restart_failed")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let paths = systemprompt_models::PathsConfig {
        system: boot.system_path.display().to_string(),
        services: root.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: None,
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .unwrap();
    let services = &app.a2a_repositories().agent_services;
    services
        .register_agent("covlister", 2_000_000_000, port)
        .await
        .expect("register an agent with a dead process id");

    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);
    let output = restart::execute_failed(&app, &config)
        .await
        .expect("a failed restart is represented in the command output");
    let value = serde_json::to_value(output.artifact()).unwrap();
    let sections = value["sections"].as_array().unwrap();
    let count = |name: &str| {
        sections
            .iter()
            .find(|section| section["heading"] == name)
            .unwrap_or_else(|| panic!("{value}"))["content"]
            .as_u64()
            .unwrap()
    };

    assert_eq!(count("restarted_count"), 0, "{value}");
    assert_eq!(count("failed_count"), 1, "{value}");
    let row = services
        .get_agent_status("covlister")
        .await
        .unwrap()
        .expect("failed restart retains its service record");
    assert_eq!(row.status, "error");
    assert_eq!(row.port, i32::from(port));
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_ok(),
        "restart failure must not terminate the unrelated listener that owns the configured port"
    );
    drop(listener);
    drop(app);
    drop(pool);
    database.drop_now().await;
}


#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

fn parse(args: &[&str]) -> AgentsCommands {
    Harness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    boot.services_path.clone()
}

fn seed_agents() -> PathBuf {
    let root = services_root();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", 9101, "Coverage Lister", true),
    )
    .unwrap();
    std::fs::write(
        root.join("agents/covdormant.yaml"),
        agent_yaml("covdormant", 9102, "Coverage Dormant", false),
    )
    .unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "includes:\n  - ../agents/covlister.yaml\n  - ../agents/covdormant.yaml\nmcp_servers: {}\n",
    )
    .unwrap();
    systemprompt_test_fixtures::refresh_services_config();
    root
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn list_renders_all_enabled_and_disabled_agents() {
    seed_agents();

    run(&["list"]).await.unwrap();
    run(&["list", "--enabled"]).await.unwrap();
    run(&["list", "--disabled"]).await.unwrap();
}

#[tokio::test]
async fn list_with_a_name_shows_that_agent_and_rejects_unknown_ones() {
    seed_agents();

    run(&["list", "covlister"]).await.unwrap();

    let err = run(&["list", "ghost"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("'ghost' not found"));
}

#[tokio::test]
async fn show_renders_a_seeded_agent_and_rejects_unknown_ones() {
    seed_agents();

    run(&["show", "covdormant"]).await.unwrap();

    let err = run(&["show", "ghost"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("ghost"));
}

#[tokio::test]
async fn validate_accepts_the_seeded_configuration() {
    seed_agents();

    run(&["validate"]).await.unwrap();
}

#[tokio::test]
async fn create_writes_a_new_agent_definition() {
    let root = seed_agents();

    run(&[
        "create",
        "--name",
        "covcreated",
        "--provider",
        "anthropic",
        "--port",
        "9133",
        "--display-name",
        "Coverage Created",
        "--description",
        "Created by the coverage suite",
        "--system-prompt",
        "You are created.",
        "--enabled",
    ])
    .await
    .unwrap();

    let written = std::fs::read_to_string(root.join("agents/covcreated.yaml")).unwrap();
    assert!(written.contains("covcreated"));
    assert!(written.contains("9133"));
}

#[tokio::test]
async fn create_rejects_an_invalid_agent_name() {
    seed_agents();

    let err = run(&[
        "create",
        "--name",
        "Bad Name!",
        "--port",
        "9134",
        "--display-name",
        "Bad",
        "--description",
        "Bad",
        "--system-prompt",
        "Bad",
    ])
    .await
    .unwrap_err();

    assert!(!format!("{err:#}").is_empty());
}

#[tokio::test]
async fn create_requires_a_name_in_non_interactive_mode() {
    seed_agents();

    let err = run(&["create", "--port", "9135"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("--name is required"));
}

#[tokio::test]
async fn edit_toggles_the_enabled_flag_of_a_seeded_agent() {
    let root = seed_agents();

    run(&["edit", "covdormant", "--enable"]).await.unwrap();

    let written = std::fs::read_to_string(root.join("agents/covdormant.yaml")).unwrap();
    assert!(written.contains("enabled: true"), "{written}");
}

#[tokio::test]
async fn edit_rejects_an_unknown_agent() {
    seed_agents();

    let err = run(&["edit", "ghost", "--enable"]).await.unwrap_err();
    assert!(format!("{err:#}").contains("ghost"));
}

#[tokio::test]
async fn delete_removes_the_selected_agent_and_reloads_the_profile_config() {
    let root = seed_agents();
    let agent_file = root.join("agents/covlister.yaml");

    run(&["delete", "covlister", "--yes"]).await.unwrap();

    assert!(
        !agent_file.exists(),
        "the selected agent definition should be removed"
    );
    let includes = std::fs::read_to_string(root.join("config/config.yaml")).unwrap();
    assert!(
        !includes.contains("covlister"),
        "deletion must remove the profile include as well: {includes}"
    );
    let config = systemprompt_loader::ConfigLoader::load().unwrap();
    assert!(
        !config.agents.contains_key("covlister"),
        "the reloaded config must not retain a deleted agent"
    );
    assert!(config.agents.contains_key("covdormant"));
}

#[tokio::test]
async fn coverage_restart_populated_registry_reports_failed_starts_and_skips_disabled_agents() {
    use std::sync::Arc;
    use systemprompt_agent::services::agent_orchestration::port_service::{
        find_process_using_port, is_agent_process,
    };
    use systemprompt_cli::infrastructure::services::restart;
    use systemprompt_test_fixtures::{
        DisposableDb, fixture_app_context_with, install_test_signing_key,
    };

    let root = seed_agents();
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an available agent port");
    let port = listener.local_addr().unwrap().port();
    assert_eq!(
        find_process_using_port(port).expect("inspect owned listener"),
        Some(std::process::id()),
        "the occupied port must belong to this test process"
    );
    assert!(
        !is_agent_process(std::process::id()).expect("classify test process"),
        "production cleanup must reject the unit-test process"
    );
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", port, "Coverage Lister", true),
    )
    .unwrap();
    systemprompt_test_fixtures::refresh_services_config();
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::installed("cli_restart_all_agents")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    let paths = systemprompt_models::PathsConfig {
        system: boot.system_path.display().to_string(),
        services: root.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: None,
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .unwrap();
    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);
    let out = restart::execute_all_agents(&app, &config).await.unwrap();
    let value = serde_json::to_value(out.artifact()).unwrap();
    let sections = value["sections"].as_array().unwrap();
    let count = |name: &str| {
        sections
            .iter()
            .find(|s| s["heading"] == name)
            .unwrap_or_else(|| panic!("{value}"))["content"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(count("restarted_count"), 0);
    assert_eq!(count("failed_count"), 1);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
    let text = CliConfig::new().with_interactive(false);
    let text_out = restart::execute_all_agents(&app, &text).await.unwrap();
    let text_value = serde_json::to_value(text_out.artifact()).unwrap();
    let text_sections = text_value["sections"].as_array().unwrap();
    let text_count = |name: &str| {
        text_sections
            .iter()
            .find(|section| section["heading"] == name)
            .unwrap_or_else(|| panic!("{text_value}"))["content"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(text_count("restarted_count"), 0);
    assert_eq!(text_count("failed_count"), 1);
    drop(listener);
    assert!(
        app.a2a_repositories()
            .agent_services
            .get_agent_status("covdormant")
            .await
            .unwrap()
            .is_none()
    );
    drop(app);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

const DELETE_ALL_HELPER: &str =
    "commands::admin_agents_profile_fs::delete_all_agents_public_helper";

#[tokio::test]
#[ignore = "re-executed by delete_all_reports_exact_targets_and_preserves_unrelated_configuration"]
async fn delete_all_agents_public_helper() {
    use std::sync::Arc;
    use systemprompt_test_fixtures::{
        DisposableDb, fixture_app_context_with, install_test_signing_key,
    };

    let root = seed_agents();
    let control = root.join("config/control.yaml");
    std::fs::write(&control, "control: preserved\n").expect("write unrelated control");
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::installed("cli_admin_delete_all")
        .await
        .expect("private delete-all database");
    let pool = database.pool().await.expect("private delete-all pool");
    let paths = systemprompt_models::PathsConfig {
        system: boot.system_path.display().to_string(),
        services: root.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: None,
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .expect("full private app context");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    println!("BEGIN_DELETE_ALL");
    execute(parse(&["delete", "--all", "--yes"]), &context)
        .await
        .expect("public delete-all command");
    println!("END_DELETE_ALL");
    let includes = std::fs::read_to_string(root.join("config/config.yaml"))
        .expect("read includes after delete-all");
    println!(
        "DELETE_ALL_STATE={}",
        serde_json::json!({
            "lister_exists": root.join("agents/covlister.yaml").exists(),
            "dormant_exists": root.join("agents/covdormant.yaml").exists(),
            "includes": includes,
            "control": std::fs::read_to_string(control).expect("read unrelated control"),
        })
    );
    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

#[test]
fn delete_all_reports_exact_targets_and_preserves_unrelated_configuration() {
    use std::io::{Read, Seek, SeekFrom};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let stdout = tempfile::NamedTempFile::new().expect("delete-all stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("delete-all stderr capture");
    let mut child = Command::new(std::env::current_exe().expect("unit-test binary"))
        .args(["--exact", DELETE_ALL_HELPER, "--ignored", "--nocapture"])
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")))
        .spawn()
        .expect("spawn delete-all helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll delete-all helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("delete-all helper timed out");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |mut file: tempfile::NamedTempFile| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read capture");
        text
    };
    let stdout = read(stdout);
    let stderr = read(stderr);
    assert!(status.success(), "delete-all helper failed: {stderr}");
    let artifact_text = stdout
        .split_once("BEGIN_DELETE_ALL\n")
        .and_then(|(_, tail)| tail.split_once("\nEND_DELETE_ALL"))
        .map(|(artifact, _)| artifact)
        .expect("bounded delete-all JSON artifact");
    let artifact: serde_json::Value =
        serde_json::from_str(artifact_text).expect("delete-all structured output");
    assert_eq!(artifact["title"], "Delete Agent", "{artifact}");
    let sections = artifact["sections"]
        .as_array()
        .expect("delete-all sections");
    let field = |heading: &str| {
        sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
            .clone()
    };
    let mut deleted = field("deleted")
        .as_array()
        .expect("deleted agent names")
        .iter()
        .map(|name| name.as_str().expect("deleted agent name").to_owned())
        .collect::<Vec<_>>();
    deleted.sort();
    assert_eq!(deleted, ["covdormant", "covlister"]);
    assert_eq!(field("message"), "2 agent(s) deleted successfully");
    let state_text = stdout
        .split_once("DELETE_ALL_STATE=")
        .map(|(_, state)| state.lines().next().expect("state line"))
        .expect("delete-all durable state");
    let state: serde_json::Value = serde_json::from_str(state_text).expect("state JSON");
    assert_eq!(state["lister_exists"], false, "{state}");
    assert_eq!(state["dormant_exists"], false, "{state}");
    assert_eq!(state["control"], "control: preserved\n", "{state}");
    let includes: serde_yaml::Value =
        serde_yaml::from_str(state["includes"].as_str().expect("serialized includes"))
            .expect("updated include YAML");
    assert!(
        includes["includes"].is_null()
            || includes["includes"] == serde_yaml::Value::Sequence(vec![]),
        "deleted includes must be absent or semantically empty: {includes:?}"
    );
    assert!(
        includes["mcp_servers"].is_null()
            || includes["mcp_servers"] == serde_yaml::Value::Mapping(Default::default()),
        "unrelated empty MCP configuration must remain semantically empty: {includes:?}"
    );
}
