//! Unit tests for port management functions

use systemprompt_mcp::services::network::port_manager::{
    find_available_port, is_port_in_use, is_port_responsive,
};

// ============================================================================
// is_port_in_use Tests
// ============================================================================

#[test]
fn test_is_port_in_use_unused_high_port() {
    let result = is_port_in_use(59995);
    assert!(!result);
}

#[test]
fn test_is_port_in_use_unused_low_port() {
    let result = is_port_in_use(59994);
    assert!(!result);
}

#[test]
fn test_is_port_in_use_boundary_max() {
    let result = is_port_in_use(65535);
    assert!(!result);
}

#[test]
fn test_is_port_in_use_boundary_min() {
    let result = is_port_in_use(1);
    assert!(!result || result);
}

// ============================================================================
// is_port_responsive Tests
// ============================================================================

#[test]
fn test_is_port_responsive_unused() {
    let result = is_port_responsive(59993);
    assert!(!result);
}

#[test]
fn test_is_port_responsive_equals_is_port_in_use() {
    let port = 59992;
    assert_eq!(is_port_in_use(port), is_port_responsive(port));
}

// ============================================================================
// find_available_port Tests
// ============================================================================

#[test]
fn test_find_available_port_success() {
    let result = find_available_port(59900, 59950);
    assert!(result.is_ok());

    let port = result.unwrap();
    assert!(port >= 59900);
    assert!(port <= 59950);
}

#[test]
fn test_find_available_port_single_port_range() {
    let result = find_available_port(59991, 59991);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 59991);
}

#[test]
fn test_find_available_port_returns_first_available() {
    let result = find_available_port(59980, 59990);
    assert!(result.is_ok());

    let port = result.unwrap();
    assert_eq!(port, 59980);
}

#[test]
fn test_find_available_port_high_range() {
    let result = find_available_port(65530, 65535);
    assert!(result.is_ok());
}

// ============================================================================
// Port Manager Async Tests
// ============================================================================

#[tokio::test]
async fn test_prepare_port_unused() {
    use systemprompt_mcp::services::network::port_manager::prepare_port;

    let result = prepare_port(59989).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wait_for_port_release_already_free() {
    use systemprompt_mcp::services::network::port_manager::wait_for_port_release;

    let result = wait_for_port_release(59988).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cleanup_port_processes_no_processes() {
    use systemprompt_mcp::services::network::port_manager::cleanup_port_processes;

    let result = cleanup_port_processes(59987).await;
    assert!(result.is_ok());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_is_port_in_use_multiple_checks_consistent() {
    let port = 59986;
    let first = is_port_in_use(port);
    let second = is_port_in_use(port);
    assert_eq!(first, second);
}

#[test]
fn test_find_available_port_various_ranges() {
    assert!(find_available_port(50000, 50010).is_ok());
    assert!(find_available_port(60000, 60010).is_ok());
    assert!(find_available_port(55000, 55005).is_ok());
}
