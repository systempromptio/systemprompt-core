use anyhow::Result;
use systemprompt_agent::services::agent_orchestration::port_service::{
    find_process_using_port, get_process_info, is_agent_process,
};
use systemprompt_agent::services::agent_orchestration::{PortService, process};

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
async fn port_service_kill_process_on_unused_port_is_noop() -> Result<()> {
    let svc = PortService::new();
    let port = free_port();
    let killed = svc.kill_process_on_port(port).await?;
    assert!(!killed);
    Ok(())
}

#[tokio::test]
async fn port_service_cleanup_port_if_needed_on_free_port_is_ok() -> Result<()> {
    let svc = PortService::new();
    let port = free_port();
    svc.cleanup_port_if_needed(port).await?;
    Ok(())
}

#[test]
fn process_is_port_in_use_is_false_for_random_high_port() {
    let port = free_port();
    assert!(!process::is_port_in_use(port));
}

#[test]
fn process_exists_for_current_pid_is_true_and_nonexistent_is_false() {
    let me = std::process::id();
    assert!(process::process_exists(me));
    assert!(!process::process_exists(1));
}

#[test]
fn probe_find_process_using_unused_port_returns_none() {
    let port = free_port();
    let res = find_process_using_port(port).expect("probe ok");
    assert!(res.is_none());
}

#[test]
fn probe_get_process_info_for_current_pid_returns_some() {
    let info = get_process_info(std::process::id()).expect("info ok");
    assert!(info.is_some());
}

#[test]
fn probe_is_agent_process_false_for_current_test_pid() {
    let is_agent = is_agent_process(std::process::id()).expect("ok");
    assert!(!is_agent);
}
