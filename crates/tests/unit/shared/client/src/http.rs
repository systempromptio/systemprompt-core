//! Unit tests for HTTP utility functions
//!
//! Tests cover:
//! - GET request handling (success, error responses, auth headers)
//! - POST request handling (with body, auth headers)
//! - PUT request handling (success, error responses)
//! - DELETE request handling (success, error responses)
//! - Error response parsing

#[cfg(test)]
use systemprompt_client::SystempromptClient;
#[cfg(test)]
use systemprompt_identifiers::JwtToken;
#[cfg(test)]
use wiremock::matchers::{body_json, header, method, path};
#[cfg(test)]
use wiremock::{Mock, MockServer, ResponseTemplate};

// Note: The http module is private, so we test it indirectly through SystempromptClient
// These tests focus on the HTTP behavior patterns

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
// GET Request Tests (via SystempromptClient)
// ============================================================================

#[tokio::test]
async fn test_get_request_success_json_response() {
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
    let result = client.list_agents().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_request_with_auth_token() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .and(header("Authorization", "Bearer test-auth-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("test-auth-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    let result = client.list_agents().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_request_401_unauthorized() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid token"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // ClientError::ApiError display format: "API error: {status} - {message}"
    let err_str = err.to_string();
    assert!(err_str.contains("401"), "Expected error to contain '401', got: {}", err_str);
}

#[tokio::test]
async fn test_get_request_404_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("404"));
}

#[tokio::test]
async fn test_get_request_500_server_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("500"));
}

#[tokio::test]
async fn test_get_request_invalid_json_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    // Just check that it's an error - the exact message format may vary
}

// ============================================================================
// POST Request Tests (via SystempromptClient)
// ============================================================================

#[tokio::test]
async fn test_post_request_success() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "name": null
    });

    // SingleResponse<UserContext> format
    let response_body = serde_json::json!({
        "data": {
            "context_id": "ctx-new-123",
            "user_id": "user-456",
            "name": "Auto-generated",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "meta": response_meta()
    });

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.create_context(None).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_post_request_with_body() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "name": "My Context"
    });

    let response_body = serde_json::json!({
        "data": {
            "context_id": "ctx-new-123",
            "user_id": "user-456",
            "name": "My Context",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "meta": response_meta()
    });

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(&response_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.create_context(Some("My Context")).await;

    assert!(result.is_ok());
    let context = result.unwrap();
    assert_eq!(context.name, "My Context");
}

#[tokio::test]
async fn test_post_request_content_type_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(header("Content-Type", "application/json"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": {
                    "context_id": "ctx-123",
                    "user_id": "user-456",
                    "name": "Test",
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z"
                },
                "meta": response_meta()
            })),
        )
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.create_context(None).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_post_request_422_validation_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(
            ResponseTemplate::new(422).set_body_string("Validation failed: name too long"),
        )
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.create_context(Some("test")).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("422"));
}

// ============================================================================
// PUT Request Tests (via SystempromptClient)
// ============================================================================

#[tokio::test]
async fn test_put_request_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/api/v1/core/contexts/ctx-123"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.update_context_name("ctx-123", "New Name").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_put_request_with_json_body() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "name": "Updated Context"
    });

    Mock::given(method("PUT"))
        .and(path("/api/v1/core/contexts/ctx-123"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .update_context_name("ctx-123", "Updated Context")
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_put_request_404_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/api/v1/core/contexts/nonexistent"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Context not found"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.update_context_name("nonexistent", "Name").await;

    assert!(result.is_err());
}

// ============================================================================
// DELETE Request Tests (via SystempromptClient)
// ============================================================================

#[tokio::test]
async fn test_delete_request_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/contexts/ctx-to-delete"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_context("ctx-to-delete").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_request_with_auth() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/contexts/ctx-123"))
        .and(header("Authorization", "Bearer delete-token"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("delete-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    let result = client.delete_context("ctx-123").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_request_403_forbidden() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/contexts/protected"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_context("protected").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("403"));
}

// ============================================================================
// Error Response Body Handling Tests
// ============================================================================

#[tokio::test]
async fn test_error_response_json_body() {
    let mock_server = MockServer::start().await;

    let error_body = serde_json::json!({
        "error": "validation_error",
        "message": "Invalid input",
        "details": ["field1 is required", "field2 must be positive"]
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(400).set_body_json(&error_body))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // The error should contain the status code
    assert!(err.to_string().contains("400"));
}

#[tokio::test]
async fn test_error_response_plain_text_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service temporarily unavailable"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("503"));
}

#[tokio::test]
async fn test_error_response_empty_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(ResponseTemplate::new(502))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
}

// ============================================================================
// Network Error Tests
// ============================================================================

#[tokio::test]
async fn test_connection_refused() {
    // Use a port that's definitely not running
    let client = SystempromptClient::new("http://127.0.0.1:59998").unwrap();
    let result = client.list_agents().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // Should be an HTTP error (connection refused)
    assert!(err.to_string().contains("HTTP request failed"));
}

// ============================================================================
// Request Timeout Tests
// ============================================================================

#[tokio::test]
async fn test_request_timeout() {
    use std::time::Duration;

    let mock_server = MockServer::start().await;

    // Simulate slow response
    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [], "meta": response_meta()}))
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&mock_server)
        .await;

    // Create client with 1 second timeout
    let client = SystempromptClient::with_timeout(&mock_server.uri(), 1).unwrap();
    let result = client.list_agents().await;

    // Should fail due to timeout
    assert!(result.is_err());
}
