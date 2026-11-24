use systemprompt_api::services::server::lifecycle::agents::reconcile_agents;
use systemprompt_identifiers::AgentName;
use systemprompt_test_fixtures::AppContextSeeds as _;
use systemprompt_test_fixtures::{
    fixture_app_context_on_bootstrap, fixture_config, init_services_bootstrap,
};

fn agent_port_candidates() -> impl Iterator<Item = u16> {
    let offset = u16::try_from(std::process::id() % 1000).unwrap_or(0);
    (0..1000_u16).map(move |step| 9000 + (offset + step) % 1000)
}

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

#[tokio::test]
async fn coverage_required_agent_failure_is_retried_and_blocks_api_startup() {
    let listener = agent_port_candidates()
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("reconcile_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Required", true));
    let ctx = fixture_app_context_on_bootstrap(
        &boot,
        fixture_config(&boot.database_url),
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await
    .unwrap();
    let err = reconcile_agents(&ctx, None).await.unwrap_err();
    let message = err.to_string();
    assert!(message.contains("failed to start after retry"), "{message}");
    assert!(message.contains(&name), "{message}");
    assert!(message.contains("port"), "{message}");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
}

#[tokio::test]
async fn coverage_disabled_agent_does_not_block_api_startup_on_an_occupied_port() {
    let listener = agent_port_candidates()
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("disabled_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Disabled", false));
    let ctx = fixture_app_context_on_bootstrap(
        &boot,
        fixture_config(&boot.database_url),
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await
    .unwrap();
    assert_eq!(reconcile_agents(&ctx, None).await.unwrap(), 0);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
}

#[cfg(unix)]
const RECONCILE_MARKER_HELPER: &str =
    "services::coverage_agent_reconciliation::reconcile_marker_helper";

#[cfg(unix)]
#[test]
#[ignore = "re-executed as an owned agent process by the reconciliation test"]
fn reconcile_marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(std::time::Duration::from_secs(600));
}

#[cfg(unix)]
struct OwnedMarkedAgent {
    child: std::process::Child,
    helper: systemprompt_test_fixtures::Helper,
    name: String,
    nonce: systemprompt_identifiers::SpawnNonce,
}

#[cfg(unix)]
impl OwnedMarkedAgent {
    fn spawn(name: &str) -> Self {
        use std::process::{Command, Stdio};

        let helper = systemprompt_test_fixtures::helper(RECONCILE_MARKER_HELPER);
        let nonce = systemprompt_identifiers::SpawnNonce::generate();
        let child = Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", RECONCILE_MARKER_HELPER, "--ignored"])
            .env(
                systemprompt_test_fixtures::HELPER_READY_ENV,
                helper.ready_path(),
            )
            .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
            .env(systemprompt_models::subprocess::AGENT_NAME_ENV, name)
            .env(
                systemprompt_models::subprocess::SPAWN_NONCE_ENV,
                nonce.as_str(),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn exact marked agent helper");
        let owned = Self {
            child,
            helper,
            name: name.to_owned(),
            nonce,
        };
        owned.helper.await_ready();
        assert!(
            systemprompt_loader::subprocess::live_pid_is_subprocess(
                owned.pid(),
                &systemprompt_loader::subprocess::ChildClaim::Agent {
                    service: systemprompt_identifiers::ServiceName::new(name),
                    stamp: systemprompt_loader::subprocess::SpawnStamp::Boot(owned.nonce.clone()),
                },
            ),
            "owned child carries the exact agent identity"
        );
        owned
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(unix)]
impl Drop for OwnedMarkedAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn reconciliation_terminates_an_owned_running_agent_before_retrying_failed_startup() {
    let reservation = agent_port_candidates()
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an allowed agent port is available");
    let port = reservation
        .local_addr()
        .expect("reserved agent port")
        .port();
    let name = format!("reconcile_running_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Running cleanup", true));
    let ctx = fixture_app_context_on_bootstrap(
        &boot,
        fixture_config(&boot.database_url),
        systemprompt_test_fixtures::fixture_admitted(),
    )
    .await
    .unwrap();
    let owned = OwnedMarkedAgent::spawn(&name);

    agent_services(&ctx)
        .register_agent(
            &AgentName::new(name.as_str()),
            owned.pid(),
            &owned.nonce,
            port,
        )
        .await
        .expect("seed the owned agent as running");

    drop(reservation);
    let error = reconcile_agents(&ctx, None)
        .await
        .expect_err("the unit-test executable cannot serve an agent subprocess");
    let message = error.to_string();
    assert!(message.contains("failed to start after retry"), "{message}");
    assert!(message.contains(&name), "{message}");

    assert!(
        !systemprompt_loader::subprocess::is_running(owned.pid()).await,
        "reconciliation must have terminated the previous agent {} before retrying",
        owned.name
    );
    let row = agent_services(&ctx)
        .find_agent_status(&AgentName::new(name.as_str()))
        .await
        .expect("read terminal agent service row")
        .expect("terminal agent service row persists");
    assert_eq!(row.status.as_str(), "error");
    assert_eq!(row.pid, None);
}

#[test]
fn malformed_agent_registry_fails_the_services_load() {
    let boot = systemprompt_test_fixtures::init_unloadable_services_bootstrap(
        "http://127.0.0.1",
        "agents: [not-a-registry]\n",
    );

    let error = systemprompt_loader::ServicesState::load(&boot.boot)
        .expect_err("malformed configured agents must block boot");

    let diagnosis = error.to_string();
    assert!(
        diagnosis.contains("agents") || diagnosis.contains("sequence"),
        "registry parse diagnosis is retained: {diagnosis}"
    );
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
