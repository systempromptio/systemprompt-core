// PortService cleanup-verb tests against real sockets: a free port is a
// no-op everywhere, and a port held by this test process (a non-agent) must
// be refused rather than reclaimed — the agent-identity check protects
// unrelated listeners from being killed.

use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use tokio::net::TcpListener;

async fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

async fn free_port() -> u16 {
    let (listener, port) = held_port().await;
    drop(listener);
    port
}

#[tokio::test]
async fn kill_process_on_port_free_port_is_false() {
    let service = PortService::new();
    let killed = service
        .kill_process_on_port(free_port().await)
        .await
        .expect("free port");
    assert!(!killed);
}

#[tokio::test]
async fn kill_process_on_port_non_agent_holder_is_refused() {
    let service = PortService::new();
    let (listener, port) = held_port().await;
    let result = service.kill_process_on_port(port).await;
    drop(listener);
    assert!(result.is_err(), "non-agent listener must not be killed");
}

#[tokio::test]
async fn cleanup_port_if_needed_free_port_is_ok() {
    let service = PortService::new();
    service
        .cleanup_port_if_needed(free_port().await)
        .await
        .expect("free port");
}

#[tokio::test]
async fn cleanup_port_if_needed_non_agent_holder_is_refused() {
    let service = PortService::new();
    let (listener, port) = held_port().await;
    let result = service.cleanup_port_if_needed(port).await;
    drop(listener);
    assert!(result.is_err());
}

#[tokio::test]
async fn wait_for_port_available_free_port_returns_immediately() {
    let service = PortService::new();
    service
        .wait_for_port_available(free_port().await, 1)
        .await
        .expect("free port");
}

#[tokio::test]
async fn wait_for_port_available_held_port_times_out() {
    let service = PortService::new();
    let (listener, port) = held_port().await;
    let result = service.wait_for_port_available(port, 1).await;
    drop(listener);
    assert!(result.is_err());
}

#[tokio::test]
async fn cleanup_agent_ports_skips_free_and_fails_on_held() {
    let service = PortService::new();

    let cleaned = service
        .cleanup_agent_ports(&[free_port().await, free_port().await])
        .await
        .expect("free ports");
    assert_eq!(cleaned, 0);

    let (listener, port) = held_port().await;
    let result = service.cleanup_agent_ports(&[port]).await;
    drop(listener);
    assert!(result.is_err());
}

#[tokio::test]
async fn verify_all_ports_available_reports_blocked_ports() {
    PortService::verify_all_ports_available(&[free_port().await]).expect("free port");

    let (listener, port) = held_port().await;
    let result = PortService::verify_all_ports_available(&[port]);
    drop(listener);
    let err = result.expect_err("held port must be reported");
    assert!(err.to_string().contains(&port.to_string()));
}
