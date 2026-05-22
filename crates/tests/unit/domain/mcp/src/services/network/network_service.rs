//! Unit tests for NetworkService

use systemprompt_mcp::services::network::NetworkService;

#[test]
fn test_network_manager_new() {
    let manager = NetworkService::new();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("NetworkService"));
}

#[test]
fn test_network_manager_default() {
    let manager = NetworkService::default();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("NetworkService"));
}

#[test]
fn test_network_manager_clone() {
    let manager = NetworkService::new();
    let cloned = manager.clone();
    let debug = format!("{:?}", cloned);
    assert!(debug.contains("NetworkService"));
}

#[test]
fn test_network_manager_is_port_responsive_unused_port() {
    let result = NetworkService::is_port_responsive(59997);
    assert!(!result);
}

#[test]
fn test_network_manager_create_router() {
    let router = NetworkService::create_router();
    let debug = format!("{:?}", router);
    assert!(debug.contains("Router"));
}

#[test]
fn test_network_manager_apply_cors_requires_config() {
    let router = NetworkService::create_router();
    let result = NetworkService::apply_cors(router);
    let _ = result;
}

#[test]
fn test_network_manager_create_proxy() {
    let router = NetworkService::create_proxy("localhost", 8080);
    let debug = format!("{:?}", router);
    assert!(debug.contains("Router"));
}
