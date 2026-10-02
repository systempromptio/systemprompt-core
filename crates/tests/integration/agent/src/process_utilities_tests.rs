use anyhow::Result;
use systemprompt_agent::services::agent_orchestration::{PortService, process};
use systemprompt_identifiers::AgentName;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

#[tokio::test]
async fn port_service_wait_for_port_available_returns_immediately_when_free() -> Result<()> {
    let svc = PortService::new();
    let port = free_port();
    svc.wait_for_port_available(port, 1).await?;
    Ok(())
}

#[tokio::test]
async fn port_service_cleanup_port_if_needed_on_free_port_is_ok() -> Result<()> {
    let svc = PortService::new();
    let port = free_port();
    svc.cleanup_port_if_needed(port, &AgentName::new("utilities_free_port"))
        .await?;
    Ok(())
}

#[test]
fn process_is_port_in_use_is_false_for_random_high_port() {
    let port = free_port();
    assert!(!process::is_port_in_use(port));
}
