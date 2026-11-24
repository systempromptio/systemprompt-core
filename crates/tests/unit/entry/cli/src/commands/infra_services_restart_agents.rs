//! `infra services restart` over a services tree holding agents: a failed
//! start is reported and its service row kept, a disabled agent is skipped,
//! and an unrelated listener on the configured port is never terminated.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_extension::cli::{CliConfig, OutputFormat};

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

fn agent_services(
    app: &systemprompt_runtime::AppContext,
) -> systemprompt_agent::repository::agent_service::AgentServiceRepository {
    let instance = app
        .kernel()
        .resolved()
        .get::<systemprompt_identifiers::InstanceId>()
        .expect("the kernel seeds the instance id");
    systemprompt_agent::repository::agent_service::AgentServiceRepository::new(
        app.db_pool(),
        instance,
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

#[tokio::test]
async fn restarting_failed_agent_reports_failure_and_preserves_failed_state() {
    use systemprompt_cli::infrastructure::services::restart;
    use systemprompt_loader::subprocess::{self, ChildClaim, SpawnStamp};
    use systemprompt_test_fixtures::DisposableDb;

    let root = seed_agents();
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an available owned agent port");
    let port = listener.local_addr().unwrap().port();
    assert_eq!(
        subprocess::pids_listening_on(port).await.unwrap(),
        vec![std::process::id()],
        "the occupied port must belong to this test process"
    );
    assert!(
        !subprocess::owns(
            std::process::id(),
            &ChildClaim::Agent {
                service: systemprompt_identifiers::ServiceName::new("covlister"),
                stamp: SpawnStamp::PreNonce,
            },
        )
        .await,
        "the production port cleanup identity check must reject the unit-test process"
    );
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", port, "Coverage Lister", true),
    )
    .unwrap();

    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = DisposableDb::with_schema("cli_restart_failed").await;
    let pool = database.test_pool().await;
    let app = crate::boot_ctx::app_reloaded_on_database(boot, database.url()).await;
    let services = agent_services(&app);
    services
        .register_agent(
            &systemprompt_identifiers::AgentName::new("covlister"),
            2_000_000_000,
            &systemprompt_identifiers::SpawnNonce::generate(),
            port,
        )
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
        .find_agent_status(&systemprompt_identifiers::AgentName::new("covlister"))
        .await
        .unwrap()
        .expect("failed restart retains its service record");
    assert_eq!(row.status.as_str(), "error");
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

#[tokio::test]
async fn coverage_restart_populated_registry_reports_failed_starts_and_skips_disabled_agents() {
    use systemprompt_cli::infrastructure::services::restart;
    use systemprompt_loader::subprocess::{self, ChildClaim, SpawnStamp};
    use systemprompt_test_fixtures::DisposableDb;

    let root = seed_agents();
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an available agent port");
    let port = listener.local_addr().unwrap().port();
    assert_eq!(
        subprocess::pids_listening_on(port)
            .await
            .expect("inspect owned listener"),
        vec![std::process::id()],
        "the occupied port must belong to this test process"
    );
    assert!(
        !subprocess::owns(
            std::process::id(),
            &ChildClaim::Agent {
                service: systemprompt_identifiers::ServiceName::new("covlister"),
                stamp: SpawnStamp::PreNonce,
            },
        )
        .await,
        "production cleanup must reject the unit-test process"
    );
    std::fs::write(
        root.join("agents/covlister.yaml"),
        agent_yaml("covlister", port, "Coverage Lister", true),
    )
    .unwrap();
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = DisposableDb::with_schema("cli_restart_all_agents").await;
    let pool = database.test_pool().await;
    let app = crate::boot_ctx::app_reloaded_on_database(boot, database.url()).await;
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
        agent_services(&app)
            .find_agent_status(&systemprompt_identifiers::AgentName::new("covdormant"))
            .await
            .unwrap()
            .is_none()
    );
    drop(app);
    pool.write_pool().close().await;
    drop(pool);
    database.drop_now().await;
}
