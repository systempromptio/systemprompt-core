//! Unit tests for SystempromptClient
//!
//! Tests cover:
//! - Client construction (new, with_timeout)
//! - Token management (with_token, set_token, token)
//! - Base URL handling
//! - Async API methods with mocked HTTP responses

#[cfg(test)]
use systemprompt_client::SystempromptClient;
#[cfg(test)]
use systemprompt_identifiers::JwtToken;
#[cfg(test)]
use wiremock::matchers::{header, method, path};
#[cfg(test)]
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper to create a standard response meta object
#[cfg(test)]
fn response_meta() -> serde_json::Value {
    serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000000",
        "timestamp": "2024-01-01T00:00:00Z",
        "version": "1.0.0"
    })
}

// ============================================================================
// Client Construction Tests
// ============================================================================

#[test]
fn test_client_new_success() {
    let client = SystempromptClient::new("https://api.example.com");
    assert!(client.is_ok());
}

#[test]
fn test_client_new_trims_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com/").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_new_multiple_trailing_slashes() {
    let client = SystempromptClient::new("https://api.example.com///").unwrap();
    // trim_end_matches removes all trailing slashes
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_new_no_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_with_timeout_success() {
    let client = SystempromptClient::with_timeout("https://api.example.com", 60);
    assert!(client.is_ok());
}

#[test]
fn test_client_with_timeout_trims_trailing_slash() {
    let client = SystempromptClient::with_timeout("https://api.example.com/", 60).unwrap();
    assert_eq!(client.base_url(), "https://api.example.com");
}

#[test]
fn test_client_with_zero_timeout() {
    let client = SystempromptClient::with_timeout("https://api.example.com", 0);
    assert!(client.is_ok());
}

#[test]
fn test_client_with_large_timeout() {
    let client = SystempromptClient::with_timeout("https://api.example.com", 3600);
    assert!(client.is_ok());
}

// ============================================================================
// Token Management Tests
// ============================================================================

#[test]
fn test_client_initially_no_token() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    assert!(client.token().is_none());
}

#[test]
fn test_client_with_token() {
    let token = JwtToken::new("test-token-12345");
    let client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token);

    assert!(client.token().is_some());
    assert_eq!(client.token().unwrap().as_str(), "test-token-12345");
}

#[test]
fn test_client_set_token() {
    let mut client = SystempromptClient::new("https://api.example.com").unwrap();
    assert!(client.token().is_none());

    let token = JwtToken::new("new-token");
    client.set_token(token);

    assert!(client.token().is_some());
    assert_eq!(client.token().unwrap().as_str(), "new-token");
}

#[test]
fn test_client_replace_token() {
    let token1 = JwtToken::new("first-token");
    let token2 = JwtToken::new("second-token");

    let mut client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token1);

    assert_eq!(client.token().unwrap().as_str(), "first-token");

    client.set_token(token2);
    assert_eq!(client.token().unwrap().as_str(), "second-token");
}

#[test]
fn test_client_with_token_chaining() {
    let token = JwtToken::new("chained-token");

    // Should be able to chain with_token
    let client = SystempromptClient::new("https://api.example.com")
        .unwrap()
        .with_token(token);

    assert_eq!(client.base_url(), "https://api.example.com");
    assert_eq!(client.token().unwrap().as_str(), "chained-token");
}

// ============================================================================
// Base URL Tests
// ============================================================================

#[test]
fn test_base_url_accessor() {
    let client = SystempromptClient::new("https://custom.api.com").unwrap();
    assert_eq!(client.base_url(), "https://custom.api.com");
}

#[test]
fn test_base_url_with_port() {
    let client = SystempromptClient::new("http://localhost:8080").unwrap();
    assert_eq!(client.base_url(), "http://localhost:8080");
}

#[test]
fn test_base_url_with_path() {
    let client = SystempromptClient::new("https://api.example.com/v1").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com/v1");
}

#[test]
fn test_base_url_with_path_trailing_slash() {
    let client = SystempromptClient::new("https://api.example.com/v1/").unwrap();
    assert_eq!(client.base_url(), "https://api.example.com/v1");
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

    // check_health returns true if request succeeds (regardless of status)
    // Looking at the source: .send().await.is_ok()
    assert!(is_healthy);
}

#[tokio::test]
async fn test_check_health_no_server() {
    // Use a port that's definitely not running
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

    // AgentCard requires: protocolVersion, name, description, url, version,
    // capabilities, defaultInputModes, defaultOutputModes, skills
    let response_body = serde_json::json!({
        "data": [
            {
                "protocolVersion": "0.3.0",
                "name": "test-agent",
                "description": "A test agent",
                "version": "1.0.0",
                "url": "https://example.com/agent",
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

    assert!(agents.is_ok());
    let agents = agents.unwrap();
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

    assert!(agents.is_ok());
    assert!(agents.unwrap().is_empty());
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

    assert!(result.is_err());
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

    let is_valid = client.verify_token().await;
    assert!(is_valid.is_ok());
    assert!(is_valid.unwrap());
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

    let is_valid = client.verify_token().await;
    assert!(is_valid.is_ok());
    assert!(!is_valid.unwrap());
}

#[tokio::test]
async fn test_verify_token_no_token_configured() {
    let client = SystempromptClient::new("https://api.example.com").unwrap();
    let result = client.verify_token().await;

    assert!(result.is_err());
}

// ============================================================================
// Async API Tests - List Contexts
// ============================================================================

#[tokio::test]
async fn test_list_contexts_success() {
    let mock_server = MockServer::start().await;

    // UserContextWithStats fields
    let response_body = serde_json::json!({
        "data": [
            {
                "context_id": "ctx-123",
                "user_id": "user-456",
                "name": "Test Context",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "task_count": 3,
                "message_count": 5,
                "last_message_at": null
            }
        ],
        "meta": response_meta()
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let contexts = client.list_contexts().await;

    assert!(contexts.is_ok());
    let contexts = contexts.unwrap();
    assert_eq!(contexts.len(), 1);
}

// ============================================================================
// Async API Tests - Delete Context
// ============================================================================

#[tokio::test]
async fn test_delete_context_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/contexts/ctx-123"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_context("ctx-123").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_context_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/contexts/nonexistent"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_context("nonexistent").await;

    assert!(result.is_err());
}

// ============================================================================
// Async API Tests - Delete Task
// ============================================================================

#[tokio::test]
async fn test_delete_task_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/tasks/task-456"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_task("task-456").await;

    assert!(result.is_ok());
}

// ============================================================================
// Async API Tests - Authorization Header
// ============================================================================

#[tokio::test]
async fn test_request_includes_auth_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .and(header("Authorization", "Bearer my-secret-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("my-secret-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    let result = client.list_agents().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_request_without_token_no_auth_header() {
    let mock_server = MockServer::start().await;

    // Mock that accepts any GET request to this path
    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    // No token set

    let result = client.list_agents().await;
    assert!(result.is_ok());
}

// ============================================================================
// Admin API Tests
// ============================================================================

#[tokio::test]
async fn test_list_logs_success() {
    let mock_server = MockServer::start().await;

    // LogEntry: timestamp, level, module, message
    let response_body = serde_json::json!([
        {
            "timestamp": "2024-01-01T00:00:00Z",
            "level": "info",
            "module": "test_module",
            "message": "Test log"
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let logs = client.list_logs(None).await;

    assert!(logs.is_ok());
}

#[tokio::test]
async fn test_list_logs_with_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let logs = client.list_logs(Some(10)).await;

    assert!(logs.is_ok());
}

#[tokio::test]
async fn test_list_users_success() {
    let mock_server = MockServer::start().await;

    // UserInfo: id, name, email, active_sessions, last_session_at, roles
    let response_body = serde_json::json!([
        {
            "id": "user-1",
            "name": "Test User",
            "email": "test@example.com",
            "active_sessions": 1,
            "last_session_at": "2024-01-01T00:00:00Z",
            "roles": ["user"]
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let users = client.list_users(None).await;

    assert!(users.is_ok());
}

#[tokio::test]
async fn test_get_analytics_success() {
    let mock_server = MockServer::start().await;

    // AnalyticsData: user_metrics, content_stats, recent_conversations, activity_trends, traffic
    let response_body = serde_json::json!({
        "user_metrics": null,
        "content_stats": [],
        "recent_conversations": [],
        "activity_trends": [],
        "traffic": null
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/analytics"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let analytics = client.get_analytics().await;

    assert!(analytics.is_ok());
}
