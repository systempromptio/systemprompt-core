//! Tests for GET and POST request handling

#[cfg(test)]
use systemprompt_client::SystempromptClient;
#[cfg(test)]
use systemprompt_identifiers::JwtToken;
#[cfg(test)]
use wiremock::matchers::{body_json, header, method, path};
#[cfg(test)]
use wiremock::{Mock, MockServer, ResponseTemplate};

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

    result.expect("GET request should succeed");
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

    client
        .list_agents()
        .await
        .expect("request with auth should succeed");
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

    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("401"),
        "Expected error to contain '401', got: {}",
        err_str
    );
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

    result.unwrap_err();
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

    result.expect("POST request should succeed");
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

    let context = result.expect("POST with body should succeed");
    assert_eq!(context.name, "My Context");
}

#[tokio::test]
async fn test_post_request_content_type_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "data": {
                "context_id": "ctx-123",
                "user_id": "user-456",
                "name": "Test",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            },
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.create_context(None).await;

    result.expect("POST with content-type should succeed");
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

    let err = result.unwrap_err();
    assert!(err.to_string().contains("422"));
}
