use systemprompt_api::services::server::lifecycle::agents::reconcile_agents;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_db_pool, init_services_bootstrap, install_test_signing_key,
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
async fn coverage_required_agent_failure_is_retried_and_blocks_api_startup() {
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("reconcile_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Required", true));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).unwrap();
    let err = reconcile_agents(&ctx, None).await.unwrap_err();
    let message = err.to_string();
    assert!(message.contains("failed to start after retry"), "{message}");
    assert!(message.contains(&name), "{message}");
    assert!(message.contains("port"), "{message}");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
}

#[tokio::test]
async fn coverage_disabled_agent_does_not_block_api_startup_on_an_occupied_port() {
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("disabled_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Disabled", false));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).unwrap();
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
}

#[cfg(unix)]
impl OwnedMarkedAgent {
    fn spawn(name: &str) -> Self {
        use std::process::{Command, Stdio};

        let helper = systemprompt_test_fixtures::helper(RECONCILE_MARKER_HELPER);
        let child = Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", RECONCILE_MARKER_HELPER, "--ignored"])
            .env(
                systemprompt_test_fixtures::HELPER_READY_ENV,
                helper.ready_path(),
            )
            .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
            .env(systemprompt_models::subprocess::AGENT_NAME_ENV, name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn exact marked agent helper");
        let owned = Self {
            child,
            helper,
            name: name.to_owned(),
        };
        owned.helper.await_ready();
        assert!(
            systemprompt_loader::subprocess::live_pid_is_subprocess(
                owned.pid(),
                systemprompt_models::subprocess::AGENT_NAME_ENV,
                name,
            ),
            "owned child carries the exact agent identity"
        );
        owned
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn wait_for_production_termination(&mut self) -> std::process::ExitStatus {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait().expect("poll owned agent child") {
                return status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "reconciliation did not terminate owned agent {}",
                self.name
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
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
    let reservation = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .expect("an allowed agent port is available");
    let port = reservation
        .local_addr()
        .expect("reserved agent port")
        .port();
    let name = format!("reconcile_running_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Running cleanup", true));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("agent reconciliation database fixture");
    let ctx = fixture_app_context(&pool, &boot.database_url)
        .expect("agent reconciliation application fixture");
    let mut owned = OwnedMarkedAgent::spawn(&name);

    ctx.a2a_repositories()
        .agent_services
        .register_agent(&name, owned.pid(), port)
        .await
        .expect("seed the owned agent as running");

    drop(reservation);
    let error = reconcile_agents(&ctx, None)
        .await
        .expect_err("the unit-test executable cannot serve an agent subprocess");
    let message = error.to_string();
    assert!(message.contains("failed to start after retry"), "{message}");
    assert!(message.contains(&name), "{message}");

    let status = owned.wait_for_production_termination();
    assert!(
        !status.success(),
        "reconciliation must terminate the previous agent rather than let it exit normally"
    );
    let row = ctx
        .a2a_repositories()
        .agent_services
        .get_agent_status(&name)
        .await
        .expect("read terminal agent service row")
        .expect("terminal agent service row persists");
    assert_eq!(row.status, "error");
    assert_eq!(row.pid, None);
}

#[tokio::test]
async fn malformed_agent_registry_fails_startup_and_emits_correlated_fatal_event() {
    use futures_util::StreamExt;
    use systemprompt_traits::StartupEvent;

    let _boot = systemprompt_test_fixtures::init_unloadable_services_bootstrap(
        "http://127.0.0.1",
        "agents: [not-a-registry]\n",
    );
    install_test_signing_key();
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("malformed_agent_registry_startup")
            .await
            .expect("private agent reconciliation database");
    let pool = database.pool().await.expect("private agent pool");
    let ctx = fixture_app_context(&pool, database.url())
        .expect("agent reconciliation application fixture");
    let (events, mut receiver) = systemprompt_traits::startup_channel();

    let error = reconcile_agents(&ctx, Some(&events))
        .await
        .expect_err("malformed configured agents must block API startup");
    let diagnosis = error.to_string();
    assert!(
        diagnosis.contains("agents") || diagnosis.contains("sequence"),
        "registry parse diagnosis is retained: {diagnosis}"
    );
    let (message, fatal) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let StartupEvent::Error { message, fatal } = receiver
                .next()
                .await
                .expect("startup event channel remains open")
            {
                break (message, fatal);
            }
        }
    })
    .await
    .expect("fatal registry event arrives");
    assert!(fatal);
    assert!(
        message.contains("Failed to load agent registry"),
        "{message}"
    );
    assert!(
        message.contains("agents") || message.contains("sequence"),
        "event preserves the registry parse diagnosis: {message}"
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM services WHERE module_name = 'agent'")
        .fetch_one(pool.pool_arc().expect("agent pool").as_ref())
        .await
        .expect("agent service count");
    assert_eq!(
        rows, 0,
        "registry failure starts and persists no agent services"
    );
    drop(ctx);
    pool.write_pool_arc().expect("agent pool").close().await;
    database.drop_now().await;
}
