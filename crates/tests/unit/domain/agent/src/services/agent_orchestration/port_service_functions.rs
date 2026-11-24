use std::time::Duration;

use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::subprocess::SpawnStamp;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("addr")
        .port()
}

#[tokio::test]
async fn port_service_wait_for_port_available_succeeds_when_free() {
    let svc = PortService::new(Duration::from_secs(5));
    svc.wait_for_port_available(free_port(), 1)
        .await
        .expect("free port must be reported available");
}

#[tokio::test]
async fn port_service_cleanup_port_if_not_in_use_returns_ok() {
    let svc = PortService::new(Duration::from_secs(5));
    svc.cleanup_port_if_needed(
        free_port(),
        &AgentName::new("free_port_agent"),
        &SpawnStamp::Unrecorded,
    )
    .await
    .expect("cleanup of a free port must succeed");
}

#[test]
fn port_service_is_built_with_its_stop_grace() {
    let _ = PortService::new(Duration::from_secs(5));
}

#[test]
fn port_service_debug() {
    let svc = PortService::new(Duration::from_secs(5));
    assert!(format!("{:?}", svc).contains("PortService"));
}
