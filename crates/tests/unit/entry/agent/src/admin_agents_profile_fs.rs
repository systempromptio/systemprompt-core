//! Profile-backed tests for the `admin agents` config commands.
//!
//! `list`, `show`, `validate`, `create`, and `edit` all read (and write) the
//! services config the bootstrapped profile points at, so the fixture tree is
//! seeded per test and the create/edit paths asserted against the files they
//! produce.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_agent_entry::cli::agents::execute;
use systemprompt_cli::EnvOverrides;

use crate::common::{
    agents_host, app_reloaded_on_database, json_cli, parse_agents, rendered,
    with_app, with_boot,
};

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
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
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
    root
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    let ctx = with_boot(json_cli());
    execute(parse_agents(args), &agents_host(&ctx)).await.map(drop)
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
async fn validate_fails_when_an_enabled_agent_names_an_unconfigured_provider() {
    seed_agents();

    let err = run(&["validate"]).await.unwrap_err();
    assert!(
        format!("{err:#}").contains("Agent validation failed"),
        "{err:#}"
    );
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
async fn create_refuses_the_reserved_registry_name() {
    let root = seed_agents();

    let err = run(&[
        "create",
        "--name",
        "registry",
        "--port",
        "9136",
        "--display-name",
        "Registry",
        "--description",
        "Shadows the agent registry",
        "--system-prompt",
        "Never created.",
    ])
    .await
    .unwrap_err();

    assert!(format!("{err:#}").contains("reserved"), "{err:#}");
    assert!(!root.join("agents/registry.yaml").exists());
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
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let app =
        app_reloaded_on_database(boot, &systemprompt_test_fixtures::test_database_url()).await;
    let context = with_app(json_cli(), EnvOverrides::default(), app);
    execute(
        parse_agents(&["delete", "covlister", "--yes"]),
        &agents_host(&context),
    )
    .await
    .unwrap();

    assert!(
        !agent_file.exists(),
        "the selected agent definition should be removed"
    );
    let includes = std::fs::read_to_string(root.join("config/config.yaml")).unwrap();
    assert!(
        !includes.contains("covlister"),
        "deletion must remove the profile include as well: {includes}"
    );
    let config = systemprompt_test_fixtures::ensure_test_bootstrap()
        .boot
        .load_services_config()
        .unwrap();
    assert!(
        !config.agents.contains_key("covlister"),
        "the reloaded config must not retain a deleted agent"
    );
    assert!(config.agents.contains_key("covdormant"));
}

const DELETE_ALL_HELPER: &str =
    "admin_agents_profile_fs::delete_all_agents_public_helper";

#[tokio::test]
#[ignore = "re-executed by delete_all_reports_exact_targets_and_preserves_unrelated_configuration"]
async fn delete_all_agents_public_helper() {
    use systemprompt_test_fixtures::DisposableDb;

    let root = seed_agents();
    let control = root.join("config/control.yaml");
    std::fs::write(&control, "control: preserved\n").expect("write unrelated control");
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = DisposableDb::with_schema("cli_admin_delete_all").await;
    let pool = database.test_pool().await;
    let app = app_reloaded_on_database(boot, database.url()).await;
    let context = with_app(json_cli(), EnvOverrides::default(), app);
    let host = agents_host(&context);

    println!("BEGIN_DELETE_ALL");
    rendered(
        &host,
        execute(parse_agents(&["delete", "--all", "--yes"]), &host).await,
    )
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
    drop(host);
    drop(context);
    pool.write_pool().close().await;
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
