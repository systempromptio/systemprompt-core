//! Tests for health check, list agents, and verify token API methods.

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn response_meta() -> serde_json::Value {
    serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000000",
        "timestamp": "2024-01-01T00:00:00Z",
        "version": "1.0.0"
    })
}

// ============================================================================
// Async API Tests - Health Check
// ============================================================================

#[tokio::test]
async fn test_check_health_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let is_healthy = client.check_health().await;

    assert!(is_healthy);
}

#[tokio::test]
async fn test_check_health_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/health"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let is_healthy = client.check_health().await;

    assert!(is_healthy);
}

#[tokio::test]
async fn test_check_health_no_server() {
    let client = SystempromptClient::new("http://127.0.0.1:59999").unwrap();
    let is_healthy = client.check_health().await;

    assert!(!is_healthy);
}

// ============================================================================
// Async API Tests - List Agents
// ============================================================================

#[tokio::test]
async fn test_list_agents_success() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "data": [
            {
                "name": "test-agent",
                "description": "A test agent",
                "version": "1.0.0",
                "supportedInterfaces": [{"url": "https://example.com/agent", "protocolBinding": "JSONRPC", "protocolVersion": "1.0.0"}],
                "capabilities": {
                    "streaming": true,
                    "pushNotifications": false,
                    "stateTransitionHistory": true
                },
                "defaultInputModes": ["text/plain"],
                "defaultOutputModes": ["text/plain"],
                "skills": []
            }
        ],
        "meta": response_meta()
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let agents = client.list_agents().await;

    let agents = agents.expect("list_agents should succeed");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "test-agent");
}

#[tokio::test]
async fn test_list_agents_empty() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "data": [],
        "meta": response_meta()
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let agents = client.list_agents().await;

    let agents = agents.expect("list_agents should succeed for empty list");
    assert!(agents.is_empty());
}

#[tokio::test]
async fn test_list_agents_unauthorized() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    result.unwrap_err();
}

// ============================================================================
// Async API Tests - Verify Token
// ============================================================================

#[tokio::test]
async fn test_verify_token_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .and(header("Authorization", "Bearer valid-token"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("valid-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    let is_valid = client.verify_token().await.expect("verify_token should succeed");
    assert!(is_valid);
}

#[tokio::test]
async fn test_verify_token_invalid() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("invalid-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    let is_valid = client.verify_token().await.expect("verify_token should succeed");
    assert!(!is_valid);
}

#[tokio::test]
async fn test_verify_token_no_token_configured() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    let result = client.verify_token().await;

    result.unwrap_err();
}
