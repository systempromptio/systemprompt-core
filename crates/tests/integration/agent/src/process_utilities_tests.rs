use std::time::Duration;

use anyhow::Result;
use systemprompt_agent::services::agent_orchestration::{PortService, process};
use systemprompt_identifiers::AgentName;
use systemprompt_loader::subprocess::SpawnStamp;

#[tokio::test]
async fn port_service_wait_for_port_available_returns_immediately_when_free() -> Result<()> {
    let svc = PortService::new(Duration::from_secs(5));
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();
    svc.wait_for_port_available(port, 1).await?;
    Ok(())
}

#[tokio::test]
async fn port_service_cleanup_port_if_needed_on_free_port_is_ok() -> Result<()> {
    let svc = PortService::new(Duration::from_secs(5));
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();
    svc.cleanup_port_if_needed(
        port,
        &AgentName::new("utilities_free_port"),
        &SpawnStamp::Unrecorded,
    )
    .await?;
    Ok(())
}

#[test]
fn process_is_port_in_use_is_false_for_random_high_port() {
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();
    assert!(!process::is_port_in_use(port));
}
