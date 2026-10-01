//! Tests for context, task, and authorization API methods.

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{ContextId, JwtToken, TaskId};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn response_meta() -> serde_json::Value {
    serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000000",
        "timestamp": "2024-01-01T00:00:00Z",
        "version": "1.0.0"
    })
}

#[tokio::test]
async fn test_list_contexts_success() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "data": [
            {
                "context_id": "00000000-0000-4000-8000-000000000001",
                "user_id": "user-456",
                "name": "Test Context",
                "kind": "user",
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

    let contexts = contexts.expect("list_contexts should succeed");
    assert_eq!(contexts.len(), 1);
}

#[tokio::test]
async fn test_delete_context_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000123",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .delete_context(
            &ContextId::try_new("00000000-0000-4000-8000-000000000123").expect("valid ContextId"),
        )
        .await;

    result.expect("delete_context should succeed");
}

#[tokio::test]
async fn test_delete_context_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-0000000000ff",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not found"))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client
        .delete_context(
            &ContextId::try_new("00000000-0000-4000-8000-0000000000ff").expect("valid ContextId"),
        )
        .await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_delete_task_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/tasks/task-456"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let result = client.delete_task(&TaskId::new("task-456")).await;

    result.expect("delete_task should succeed");
}

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

    client
        .list_agents()
        .await
        .expect("request with auth header should succeed");
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

    client
        .list_agents()
        .await
        .expect("request without token should succeed");
}
