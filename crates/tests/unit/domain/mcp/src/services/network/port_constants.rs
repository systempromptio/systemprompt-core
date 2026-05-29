use systemprompt_mcp::services::network::port::{
    MAX_PORT_CLEANUP_ATTEMPTS, PORT_BACKOFF_BASE_MS, POST_KILL_DELAY_MS, cleanup_port_resources,
    find_available_port, is_port_in_use,
};

#[test]
fn max_port_cleanup_attempts_nonzero() {
    assert!(MAX_PORT_CLEANUP_ATTEMPTS > 0);
    assert_eq!(MAX_PORT_CLEANUP_ATTEMPTS, 5);
}

#[test]
fn port_backoff_base_ms_positive() {
    assert!(PORT_BACKOFF_BASE_MS > 0);
    assert_eq!(PORT_BACKOFF_BASE_MS, 200);
}

#[test]
fn post_kill_delay_ms_positive() {
    assert!(POST_KILL_DELAY_MS > 0);
    assert_eq!(POST_KILL_DELAY_MS, 500);
}

#[test]
fn cleanup_port_resources_is_noop() {
    cleanup_port_resources(0);
    cleanup_port_resources(1024);
    cleanup_port_resources(65535);
}

#[test]
fn find_available_port_full_range() {
    let result = find_available_port(59800, 59900);
    assert!(result.is_ok());
    let port = result.unwrap();
    assert!(port >= 59800 && port <= 59900);
}

#[test]
fn find_available_port_empty_range_fails() {
    let result = find_available_port(65534, 65533);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("No available") || msg.contains("port"));
}

#[test]
fn is_port_in_use_high_unused_port() {
    assert!(!is_port_in_use(59700));
    assert!(!is_port_in_use(59701));
}

#[test]
fn is_port_in_use_returns_bool() {
    let r = is_port_in_use(59702);
    let _ = r;
}

#[test]
fn find_available_port_starts_from_given_port() {
    let result = find_available_port(59750, 59760);
    let port = result.expect("ok");
    assert_eq!(port, 59750);
}
