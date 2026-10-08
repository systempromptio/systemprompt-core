use systemprompt_agent::services::agent_orchestration::port_service::PortService;
use systemprompt_identifiers::AgentName;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("addr")
        .port()
}

#[tokio::test]
async fn port_service_wait_for_port_available_succeeds_when_free() {
    let svc = PortService::new();
    svc.wait_for_port_available(free_port(), 1)
        .await
        .expect("free port must be reported available");
}

#[tokio::test]
async fn port_service_cleanup_port_if_not_in_use_returns_ok() {
    let svc = PortService::new();
    svc.cleanup_port_if_needed(free_port(), &AgentName::new("free_port_agent"))
        .await
        .expect("cleanup of a free port must succeed");
}

#[test]
fn port_service_new_is_unit_struct() {
    let _ = PortService::new();
    let _ = PortService;
    let _ = PortService::default();
}

#[test]
fn port_service_debug() {
    let svc = PortService::new();
    assert!(format!("{:?}", svc).contains("PortService"));
}
