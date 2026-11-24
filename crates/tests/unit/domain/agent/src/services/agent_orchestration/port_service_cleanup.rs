// PortService cleanup-verb tests against real sockets: a free port is a
// no-op, and a port held by this test process (which carries no agent marker)
// must be refused rather than reclaimed — the identity check protects
// unrelated listeners from being killed.

use std::time::Duration;

use systemprompt_agent::services::agent_orchestration::OrchestrationError;
use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::subprocess::SpawnStamp;
use tokio::net::TcpListener;

async fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn cleanup_port_if_needed_free_port_is_ok() {
    let service = PortService::new(Duration::from_secs(5));
    service
        .cleanup_port_if_needed(
            systemprompt_test_fixtures::ReservedPort::loopback().release_for_child(),
            &AgentName::new("cleanup_free"),
            &SpawnStamp::Unrecorded,
        )
        .await
        .expect("free port");
}

#[tokio::test]
async fn cleanup_port_if_needed_non_agent_holder_is_refused() {
    let service = PortService::new(Duration::from_secs(5));
    let (listener, port) = held_port().await;
    let result = service
        .cleanup_port_if_needed(
            port,
            &AgentName::new("cleanup_held"),
            &SpawnStamp::Unrecorded,
        )
        .await;
    drop(listener);
    assert!(
        matches!(
            result,
            Err(OrchestrationError::PortHeldByForeignProcess { port: p, pid, .. })
                if p == port && pid == std::process::id()
        ),
        "an unmarked listener is refused by pid, never signalled: {result:?}"
    );
}

#[tokio::test]
async fn wait_for_port_available_free_port_returns_immediately() {
    let service = PortService::new(Duration::from_secs(5));
    service
        .wait_for_port_available(systemprompt_test_fixtures::ReservedPort::loopback().release_for_child(), 1)
        .await
        .expect("free port");
}

#[tokio::test]
async fn wait_for_port_available_held_port_times_out() {
    let service = PortService::new(Duration::from_secs(5));
    let (listener, port) = held_port().await;
    let result = service.wait_for_port_available(port, 1).await;
    drop(listener);
    assert!(result.is_err());
}

#[tokio::test]
async fn wait_for_port_available_observes_release_during_the_poll_window() {
    let service = PortService::new(Duration::from_secs(5));
    let (listener, port) = held_port().await;
    let release = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        drop(listener);
    });

    service
        .wait_for_port_available(port, 2)
        .await
        .expect("the released port must become available before timeout");
    release.await.expect("release task");

    let rebound = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("successful wait means the port is bindable");
    drop(rebound);
}

#[tokio::test]
async fn wait_for_port_available_timeout_names_port_and_deadline() {
    let service = PortService::new(Duration::from_secs(5));
    let (listener, port) = held_port().await;
    let err = service
        .wait_for_port_available(port, 1)
        .await
        .expect_err("timeout");
    assert!(err.to_string().contains(&format!("Port {port}")));
    assert!(err.to_string().contains("within 1 seconds"));
    drop(listener);
}
