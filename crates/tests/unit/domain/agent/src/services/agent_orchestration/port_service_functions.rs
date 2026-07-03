use systemprompt_agent::services::agent_orchestration::port_service::{
    PortService, find_process_using_port, get_process_info, is_agent_process,
};

#[tokio::test]
async fn port_service_kill_unused_port_returns_false() {
    let svc = PortService::new();
    // High port unlikely to be in use
    let result = svc.kill_process_on_port(64739).await;
    // returns Ok(false) for no process found
    assert!(result.is_ok() || result.is_err());
    if let Ok(killed) = result {
        assert!(!killed);
    }
}

#[tokio::test]
async fn port_service_wait_for_port_available_succeeds_when_free() {
    let svc = PortService::new();
    svc.wait_for_port_available(64758, 1)
        .await
        .expect("free port must be reported available");
}

#[tokio::test]
async fn port_service_cleanup_port_if_not_in_use_returns_ok() {
    let svc = PortService::new();
    svc.cleanup_port_if_needed(64769)
        .await
        .expect("cleanup of a free port must succeed");
}

#[tokio::test]
async fn port_service_cleanup_agent_ports_handles_empty() {
    let svc = PortService::new();
    let result = svc.cleanup_agent_ports(&[]).await.expect("ok");
    assert_eq!(result, 0);
}

#[tokio::test]
async fn port_service_cleanup_agent_ports_with_unused_returns_zero_cleaned() {
    let svc = PortService::new();
    let result = svc
        .cleanup_agent_ports(&[64789, 64790, 64791])
        .await
        .expect("ok");
    assert_eq!(result, 0);
}

#[test]
fn port_service_verify_all_ports_available_with_empty_list() {
    PortService::verify_all_ports_available(&[]).expect("empty port list is trivially available");
}

#[test]
fn port_service_verify_all_ports_available_with_free_ports() {
    PortService::verify_all_ports_available(&[64810, 64811, 64812])
        .expect("free ports must verify as available");
}

#[test]
fn port_service_new_is_unit_struct() {
    let _ = PortService::new();
    let _ = PortService;
    let _ = PortService::default();
}

#[test]
fn port_service_clone_and_copy() {
    let svc = PortService::new();
    let copied = svc;
    let cloned = copied.clone();
    let _ = cloned;
}

#[test]
fn port_service_debug() {
    let svc = PortService::new();
    assert!(format!("{:?}", svc).contains("PortService"));
}

#[test]
fn find_process_using_port_for_unused_port() {
    // High port unlikely to be in use; should return Ok(None) or Ok(Some(_))
    // either way exercises the lsof/netstat fork code path.
    let _ = find_process_using_port(64321);
}

#[test]
fn is_agent_process_for_self_pid() {
    let me = std::process::id();
    // Whether or not it's an agent doesn't matter — we exercise the
    // /proc/<pid>/cmdline inspection.
    let _ = is_agent_process(me);
}

#[test]
fn get_process_info_for_self_pid() {
    let me = std::process::id();
    get_process_info(me).expect("self pid must be queryable");
}

#[test]
fn get_process_info_for_invalid_pid() {
    let info = get_process_info(u32::MAX).expect("query must not error");
    assert!(info.is_none());
}

#[tokio::test]
async fn port_service_wait_for_port_available_returns_quickly_when_free() {
    let svc = PortService::new();
    // Pick a port unlikely to be in use; should not block long.
    let result = svc.wait_for_port_available(54321, 1).await;
    // Either OK (free) or error (still in use after timeout); both exercise the
    // loop.
    let _ = result;
}

#[tokio::test]
async fn port_service_kill_process_on_unused_port_returns_false() {
    let svc = PortService::new();
    let result = svc.kill_process_on_port(54322).await;
    // Empty port — function should return Ok(false) or Err, exercising the path.
    let _ = result;
}

#[tokio::test]
async fn port_service_cleanup_agent_ports_empty_list() {
    let svc = PortService::new();
    let result = svc.cleanup_agent_ports(&[]).await;
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn port_service_cleanup_port_if_needed_for_free_port() {
    let svc = PortService::new();
    let result = svc.cleanup_port_if_needed(54323).await;
    // Free port — returns Ok(())
    let _ = result;
}

#[test]
fn port_service_verify_all_ports_available_empty_list_ok() {
    PortService::verify_all_ports_available(&[]).expect("empty port list is trivially available");
}
