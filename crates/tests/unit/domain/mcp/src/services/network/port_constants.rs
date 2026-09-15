use systemprompt_mcp::services::network::port::{
    MAX_PORT_CLEANUP_ATTEMPTS, PORT_BACKOFF_BASE_MS, POST_KILL_DELAY_MS, is_port_in_use,
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

#[tokio::test]
async fn is_port_in_use_high_unused_port() {
    assert!(!is_port_in_use(59700).await);
    assert!(!is_port_in_use(59701).await);
}

#[tokio::test]
async fn is_port_in_use_returns_bool() {
    let r = is_port_in_use(59702).await;
    let _ = r;
}
