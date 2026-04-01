//! Tests for context, task, authorization, and admin API methods.

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
// Async API Tests - List Contexts
// ============================================================================

#[tokio::test]
async fn test_list_contexts_success() {
    let mock_server = MockServer::start().await;

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

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();

    let result = client.list_agents().await;
    assert!(result.is_ok());
}

// ============================================================================
// Admin API Tests
// ============================================================================

#[tokio::test]
async fn test_list_logs_success() {
    let mock_server = MockServer::start().await;

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
