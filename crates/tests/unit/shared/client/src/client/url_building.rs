//! Tests for URL building, query-string construction, and path formatting.
//!
//! These tests exercise `limited_url` (via
//! list_logs/list_users/list_all_artifacts), context/task/artifact URL
//! composition, agent card paths, and the send_message JSON-RPC envelope — all
//! without requiring a live server or touching any network by inspecting
//! wiremock request paths and query strings.

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{ContextId, JwtToken, TaskId};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn response_meta() -> serde_json::Value {
    serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000000",
        "timestamp": "2024-01-01T00:00:00Z",
        "version": "1.0.0"
    })
}

#[tokio::test]
async fn test_list_logs_no_limit_omits_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_logs(None)
        .await
        .expect("list_logs(None) should succeed");
}

#[tokio::test]
async fn test_list_logs_with_limit_appends_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/logs"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_logs(Some(50))
        .await
        .expect("list_logs(Some(50)) should succeed");
}

#[tokio::test]
async fn test_list_users_no_limit_omits_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_users(None)
        .await
        .expect("list_users(None) should succeed");
}

#[tokio::test]
async fn test_list_users_with_limit_appends_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/users"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_users(Some(100))
        .await
        .expect("list_users(Some(100)) should succeed");
}

#[tokio::test]
async fn test_list_all_artifacts_no_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/artifacts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_all_artifacts(None)
        .await
        .expect("list_all_artifacts(None) should succeed");
}

#[tokio::test]
async fn test_list_all_artifacts_with_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/artifacts"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_all_artifacts(Some(25))
        .await
        .expect("list_all_artifacts(Some(25)) should succeed");
}

#[tokio::test]
async fn test_list_logs_limit_one() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/admin/logs"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .list_logs(Some(1))
        .await
        .expect("list_logs(Some(1)) should succeed");
}

#[tokio::test]
async fn test_get_context_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000abc",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "context_id": "00000000-0000-4000-8000-000000000abc",
                "user_id": "user-1",
                "name": "Test",
                "kind": "user",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            },
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context = client
        .get_context(&ContextId::new("00000000-0000-4000-8000-000000000abc"))
        .await
        .expect("get_context should succeed");

    assert_eq!(context.name, "Test");
}

#[tokio::test]
async fn test_list_tasks_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000001/tasks",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let tasks = client
        .list_tasks(&ContextId::new("00000000-0000-4000-8000-000000000001"))
        .await
        .expect("list_tasks should succeed");

    assert!(tasks.is_empty());
}

#[tokio::test]
async fn test_list_artifacts_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000002/artifacts",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let artifacts = client
        .list_artifacts(&ContextId::new("00000000-0000-4000-8000-000000000002"))
        .await
        .expect("list_artifacts should succeed");

    assert!(artifacts.is_empty());
}

#[tokio::test]
async fn test_delete_task_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/core/tasks/task-abc-123"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .delete_task(&TaskId::new("task-abc-123"))
        .await
        .expect("delete_task should succeed");
}

#[tokio::test]
async fn test_get_agent_card_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/.well-known/agent-cards/my-agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "my-agent",
            "description": "A test agent",
            "version": "1.0.0",
            "supportedInterfaces": [],
            "capabilities": {
                "streaming": false,
                "pushNotifications": false,
                "stateTransitionHistory": false
            },
            "defaultInputModes": ["text/plain"],
            "defaultOutputModes": ["text/plain"],
            "skills": []
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let card = client
        .get_agent_card("my-agent")
        .await
        .expect("get_agent_card should succeed");

    assert_eq!(card.name, "my-agent");
}

#[tokio::test]
async fn test_send_message_builds_correct_url_and_envelope() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/agents/my-agent/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "result": {},
            "id": "00000000-0000-4000-8000-000000000001"
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let message = serde_json::json!({"role": "user", "parts": [{"text": "hello"}]});

    let result = client
        .send_message(
            "my-agent",
            &ContextId::new("00000000-0000-4000-8000-000000000001"),
            message,
        )
        .await
        .expect("send_message should succeed");

    assert!(result.get("jsonrpc").is_some());
}

#[tokio::test]
async fn test_send_message_requires_auth_header_when_token_set() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/agents/agent-x/"))
        .and(wiremock::matchers::header(
            "Authorization",
            "Bearer msg-token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "result": {},
            "id": "00000000-0000-4000-8000-0000000000c1"
        })))
        .mount(&mock_server)
        .await;

    let token = JwtToken::new("msg-token");
    let client = SystempromptClient::new(&mock_server.uri())
        .unwrap()
        .with_token(token);

    client
        .send_message(
            "agent-x",
            &ContextId::new("00000000-0000-4000-8000-0000000000c1"),
            serde_json::json!({}),
        )
        .await
        .expect("send_message with token should succeed");
}

#[tokio::test]
async fn test_update_context_name_builds_correct_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000999",
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    client
        .update_context_name(
            &ContextId::new("00000000-0000-4000-8000-000000000999"),
            "Renamed",
        )
        .await
        .expect("update_context_name should succeed");
}

#[tokio::test]
async fn test_list_contexts_includes_sort_query() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .and(query_param("sort", "updated_at:desc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let contexts = client
        .list_contexts()
        .await
        .expect("list_contexts should include sort param");

    assert!(contexts.is_empty());
}
