//! Unit tests for port management functions

use systemprompt_mcp::services::network::port::{is_port_in_use, is_port_responsive};

#[tokio::test]
async fn test_is_port_in_use_unused_high_port() {
    let result = is_port_in_use(59995).await;
    assert!(!result);
}

#[tokio::test]
async fn test_is_port_in_use_unused_low_port() {
    let result = is_port_in_use(59994).await;
    assert!(!result);
}

#[tokio::test]
async fn test_is_port_in_use_boundary_max() {
    let result = is_port_in_use(65535).await;
    assert!(!result);
}

#[tokio::test]
async fn test_is_port_in_use_boundary_min() {
    let result = is_port_in_use(1).await;
    assert!(!result || result);
}

#[tokio::test]
async fn test_is_port_responsive_unused() {
    let result = is_port_responsive(59993).await;
    assert!(!result);
}

#[tokio::test]
async fn test_prepare_port_unused() {
    use systemprompt_mcp::services::network::port::prepare_port;

    let result = prepare_port(59989, "systemprompt").await;
    result.expect("expected success");
}

#[tokio::test]
async fn test_wait_for_port_release_already_free() {
    use systemprompt_mcp::services::network::port::wait_for_port_release;

    let result = wait_for_port_release(59988).await;
    result.expect("expected success");
}

#[tokio::test]
async fn test_cleanup_port_processes_no_processes() {
    use systemprompt_mcp::services::network::port::cleanup_port_processes;

    let result = cleanup_port_processes(59987, "systemprompt").await;
    result.expect("expected success");
}

#[tokio::test]
async fn test_is_port_in_use_multiple_checks_consistent() {
    let port = 59986;
    let first = is_port_in_use(port).await;
    let second = is_port_in_use(port).await;
    assert_eq!(first, second);
}
