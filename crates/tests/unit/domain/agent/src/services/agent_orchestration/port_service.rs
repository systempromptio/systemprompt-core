// The reclaim path of `PortService`, which the refusal-oriented suites in
// `port_service_cleanup` never reach: a listener carrying this agent's spawn
// markers is a stale copy of the agent, so it is stopped and the port is then
// observed to free up; the same listener is refused for any other agent.
//
// The stand-in listener is this crate's own test binary re-executing an
// ignored helper, so its environment is readable on macOS as well as Linux.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use systemprompt_agent::services::agent_orchestration::OrchestrationError;
use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::subprocess;

const LISTENER_HELPER: &str = "services::agent_orchestration::port_service::listener_helper";
const LISTEN_PORT_ENV: &str = "SYSTEMPROMPT_TEST_LISTEN_PORT";

#[test]
#[ignore = "re-executed as a marked agent listener by the reclaim tests"]
fn listener_helper() {
    let port: u16 = std::env::var(LISTEN_PORT_ENV)
        .expect("listen port")
        .parse()
        .expect("numeric port");
    let _listener = TcpListener::bind(("127.0.0.1", port)).expect("bind helper port");
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(120));
}

fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

fn unique_agent(prefix: &str) -> AgentName {
    AgentName::new(format!("{prefix}_{}", uuid::Uuid::new_v4().simple()))
}

fn spawn_marked_listener(agent: &AgentName, port: u16) -> Child {
    let helper = systemprompt_test_fixtures::helper(LISTENER_HELPER);
    let child = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", LISTENER_HELPER, "--ignored"])
        .env(
            systemprompt_test_fixtures::HELPER_READY_ENV,
            helper.ready_path(),
        )
        .env(LISTEN_PORT_ENV, port.to_string())
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::AGENT_NAME_ENV,
            agent.as_str(),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn listener helper");
    helper.await_ready();
    child
}

async fn reap(child: Child) {
    let _ = subprocess::terminate_gracefully(child.id(), Duration::ZERO).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_port_if_needed_stops_a_stale_copy_of_this_agent() {
    let agent = unique_agent("reclaim");
    let port = reserve_port();
    let child = spawn_marked_listener(&agent, port);
    let pid = child.id();

    let result = PortService::new()
        .cleanup_port_if_needed(port, &agent)
        .await;
    let still_running = subprocess::is_running(pid).await;
    reap(child).await;

    result.expect("a port held by this agent's own stale process is reclaimed");
    assert!(!still_running, "the stale holder at pid {pid} is stopped");
    assert!(
        TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "the port is bindable once the holder is gone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_port_if_needed_refuses_a_holder_marked_for_another_agent() {
    let port = reserve_port();
    let child = spawn_marked_listener(&unique_agent("owner"), port);
    let pid = child.id();

    let result = PortService::new()
        .cleanup_port_if_needed(port, &unique_agent("other"))
        .await;
    let still_running = subprocess::is_running(pid).await;
    reap(child).await;

    assert!(
        matches!(
            result,
            Err(OrchestrationError::PortHeldByForeignProcess { pid: holder, .. }) if holder == pid
        ),
        "another agent's listener is refused by pid: {result:?}"
    );
    assert!(
        still_running,
        "a holder that is not this agent is never signalled"
    );
}
