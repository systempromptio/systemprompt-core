//! Unit tests for NetworkService

use systemprompt_mcp::services::network::NetworkService;

#[tokio::test]
async fn test_network_manager_is_port_responsive_unused_port() {
    let result = NetworkService::is_port_responsive(59997).await;
    assert!(!result);
}
