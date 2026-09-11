//! Unit tests for NetworkService

use systemprompt_mcp::services::network::NetworkService;


#[test]
fn test_network_manager_is_port_responsive_unused_port() {
    let result = NetworkService::is_port_responsive(59997);
    assert!(!result);
}


#[test]
fn test_network_manager_apply_cors_requires_config() {
    let router = NetworkService::create_router();
    let result = NetworkService::apply_cors(router);
    let _ = result;
}
