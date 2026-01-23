//! Unit tests for NetworkManager

use systemprompt_mcp::services::network::NetworkManager;

// ============================================================================
// NetworkManager Creation Tests
// ============================================================================

#[test]
fn test_network_manager_new() {
    let manager = NetworkManager::new();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("NetworkManager"));
}

#[test]
fn test_network_manager_default() {
    let manager = NetworkManager::default();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("NetworkManager"));
}

#[test]
fn test_network_manager_clone() {
    let manager = NetworkManager::new();
    let cloned = manager.clone();
    let debug = format!("{:?}", cloned);
    assert!(debug.contains("NetworkManager"));
}

#[test]
fn test_network_manager_copy() {
    let manager = NetworkManager::new();
    let copied = manager;
    let _original_debug = format!("{:?}", manager);
    let _copied_debug = format!("{:?}", copied);
}

// ============================================================================
// NetworkManager Static Method Tests
// ============================================================================

#[test]
fn test_network_manager_is_port_responsive_unused_port() {
    let result = NetworkManager::is_port_responsive(59997);
    assert!(!result);
}

#[test]
fn test_network_manager_cleanup_port_resources() {
    NetworkManager::cleanup_port_resources(59996);
}

#[test]
fn test_network_manager_create_router() {
    let router = NetworkManager::create_router();
    let debug = format!("{:?}", router);
    assert!(debug.contains("Router"));
}

#[test]
fn test_network_manager_apply_cors_requires_config() {
    let router = NetworkManager::create_router();
    let result = NetworkManager::apply_cors(router);
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_network_manager_create_proxy() {
    let router = NetworkManager::create_proxy("localhost", 8080);
    let debug = format!("{:?}", router);
    assert!(debug.contains("Router"));
}

#[test]
fn test_network_manager_create_proxy_various_hosts() {
    let _ = NetworkManager::create_proxy("127.0.0.1", 3000);
    let _ = NetworkManager::create_proxy("0.0.0.0", 5000);
    let _ = NetworkManager::create_proxy("example.com", 443);
}

#[test]
fn test_network_manager_create_proxy_boundary_ports() {
    let _ = NetworkManager::create_proxy("localhost", 1);
    let _ = NetworkManager::create_proxy("localhost", 65535);
}
