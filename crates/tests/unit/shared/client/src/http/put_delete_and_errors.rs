//! Tests for PUT, DELETE requests, error response handling, network errors, and
//! timeouts

#[cfg(test)]
use systemprompt_client::SystempromptClient;
#[cfg(test)]
use systemprompt_identifiers::{ContextId, JwtToken};
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

#[tokio::test]
async fn test_put_request_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000123",
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .update_context_name(
            &ContextId::new("00000000-0000-4000-8000-000000000123"),
            "New Name",
        )
        .await;

    result.expect("PUT request should succeed");
}

#[tokio::test]
async fn test_put_request_with_json_body() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "name": "Updated Context"
    });

    Mock::given(method("PUT"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000123",
        ))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .update_context_name(
            &ContextId::new("00000000-0000-4000-8000-000000000123"),
            "Updated Context",
        )
        .await;

    result.expect("PUT with JSON body should succeed");
}

#[tokio::test]
async fn test_put_request_404_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-0000000000ff",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_string("Context not found"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .update_context_name(
            &ContextId::new("00000000-0000-4000-8000-0000000000ff"),
            "Name",
        )
        .await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_delete_request_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000de1",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .delete_context(&ContextId::new("00000000-0000-4000-8000-000000000de1"))
        .await;

    result.expect("DELETE request should succeed");
}

#[tokio::test]
async fn test_delete_request_with_auth() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000123",
        ))
        .and(header("Authorization", "Bearer delete-token"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("delete-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    client
        .delete_context(&ContextId::new("00000000-0000-4000-8000-000000000123"))
        .await
        .expect("DELETE with auth should succeed");
}

#[tokio::test]
async fn test_delete_request_403_forbidden() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-0000000000aa",
        ))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .delete_context(&ContextId::new("00000000-0000-4000-8000-0000000000aa"))
        .await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("403"));
}

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

    let err = result.unwrap_err();
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

    result.unwrap_err();
}

#[tokio::test]
async fn test_connection_refused() {
    let client = SystempromptClient::new("http://127.0.0.1:59998").unwrap();
    let result = client.list_agents().await;

    let err = result.unwrap_err();
    assert!(err.to_string().contains("HTTP request failed"));
}

#[tokio::test]
async fn test_request_timeout() {
    use std::time::Duration;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/agents/registry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [], "meta": response_meta()}))
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::with_timeout(&mock_server.uri(), 1).unwrap();
    let result = client.list_agents().await;

    result.unwrap_err();
}

#[tokio::test]
async fn error_response_with_unreadable_body_still_reports_the_status() {
    use tokio::io::AsyncWriteExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        sock.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4096\r\n\r\ntrunc")
            .await
            .unwrap();
        sock.shutdown().await.ok();
    });

    let client = SystempromptClient::new(&format!("http://{addr}")).unwrap();
    let err = client
        .list_agents()
        .await
        .expect_err("500 with a truncated body is an error");
    match err {
        systemprompt_client::ClientError::ApiError {
            status, message, ..
        } => {
            assert_eq!(status, 500);
            assert!(
                message.contains("body unreadable"),
                "message should flag the unreadable body, got: {message}"
            );
        },
        other => panic!("expected ApiError, got {other:?}"),
    }
}
