//! Tests for fetch_or_create_context, create_context_auto_name, and
//! scenario-level interactions between client methods.

use systemprompt_client::SystempromptClient;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn response_meta() -> serde_json::Value {
    serde_json::json!({
        "request_id": "00000000-0000-0000-0000-000000000000",
        "timestamp": "2024-01-01T00:00:00Z",
        "version": "1.0.0"
    })
}

fn make_context(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "context_id": id,
        "user_id": "user-1",
        "name": name,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    })
}

fn make_context_with_stats(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "context_id": id,
        "user_id": "user-1",
        "name": name,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "task_count": 0,
        "message_count": 0,
        "last_message_at": null
    })
}

#[tokio::test]
async fn test_fetch_or_create_context_returns_existing_when_present() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .and(query_param("sort", "updated_at:desc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [make_context_with_stats("00000000-0000-4000-8000-000000000011", "Existing")],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context_id = client
        .fetch_or_create_context()
        .await
        .expect("fetch_or_create should succeed with existing context");

    assert_eq!(context_id.as_str(), "00000000-0000-4000-8000-000000000011");
}

#[tokio::test]
async fn test_fetch_or_create_context_creates_when_list_is_empty() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "data": make_context("00000000-0000-4000-8000-000000000022", "Session 2024-01-01 00:00"),
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context_id = client
        .fetch_or_create_context()
        .await
        .expect("fetch_or_create should create when no contexts exist");

    assert_eq!(context_id.as_str(), "00000000-0000-4000-8000-000000000022");
}

#[tokio::test]
async fn test_fetch_or_create_context_returns_first_context_only() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                make_context_with_stats("00000000-0000-4000-8000-000000000033", "First"),
                make_context_with_stats("00000000-0000-4000-8000-000000000044", "Second")
            ],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context_id = client
        .fetch_or_create_context()
        .await
        .expect("fetch_or_create should return first context");

    assert_eq!(context_id.as_str(), "00000000-0000-4000-8000-000000000033");
}

#[tokio::test]
async fn test_create_context_auto_name_posts_to_contexts() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "data": make_context("00000000-0000-4000-8000-000000000055", "Session 2024-01-01 12:00"),
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context = client
        .create_context_auto_name()
        .await
        .expect("create_context_auto_name should succeed");

    assert_eq!(
        context.context_id.as_str(),
        "00000000-0000-4000-8000-000000000055"
    );
}

#[tokio::test]
async fn test_create_context_with_name_sends_name() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({ "name": "My Session" });

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "data": make_context("00000000-0000-4000-8000-000000000066", "My Session"),
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context = client
        .create_context(Some("My Session"))
        .await
        .expect("create_context with name should succeed");

    assert_eq!(context.name, "My Session");
}

#[tokio::test]
async fn test_create_context_without_name_sends_null() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({ "name": null });

    Mock::given(method("POST"))
        .and(path("/api/v1/core/contexts"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "data": make_context("00000000-0000-4000-8000-000000000077", "Auto"),
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context = client
        .create_context(None)
        .await
        .expect("create_context without name should succeed");

    assert_eq!(
        context.context_id.as_str(),
        "00000000-0000-4000-8000-000000000077"
    );
}

#[tokio::test]
async fn test_get_context_returns_data_field() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-000000000088",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": make_context("00000000-0000-4000-8000-000000000088", "Fetched Context"),
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let context = client
        .get_context(&systemprompt_identifiers::ContextId::new(
            "00000000-0000-4000-8000-000000000088",
        ))
        .await
        .expect("get_context should succeed");

    assert_eq!(context.name, "Fetched Context");
    assert_eq!(
        context.context_id.as_str(),
        "00000000-0000-4000-8000-000000000088"
    );
}

#[tokio::test]
async fn test_list_contexts_returns_all_items() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/core/contexts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                make_context_with_stats("00000000-0000-4000-8000-0000000000a1", "Alpha"),
                make_context_with_stats("00000000-0000-4000-8000-0000000000a2", "Beta"),
                make_context_with_stats("00000000-0000-4000-8000-0000000000a3", "Gamma"),
            ],
            "meta": response_meta()
        })))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let contexts = client
        .list_contexts()
        .await
        .expect("list_contexts should return all items");

    assert_eq!(contexts.len(), 3);
    assert_eq!(contexts[0].name, "Alpha");
    assert_eq!(contexts[2].name, "Gamma");
}

#[tokio::test]
async fn test_list_artifacts_returns_json_values() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/core/contexts/00000000-0000-4000-8000-0000000000b1/artifacts",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "art-1", "type": "text"},
            {"id": "art-2", "type": "image"}
        ])))
        .mount(&mock_server)
        .await;

    let client = SystempromptClient::new(&mock_server.uri()).unwrap();
    let artifacts = client
        .list_artifacts(&systemprompt_identifiers::ContextId::new(
            "00000000-0000-4000-8000-0000000000b1",
        ))
        .await
        .expect("list_artifacts should return JSON values");

    assert_eq!(artifacts.len(), 2);
    assert_eq!(artifacts[0]["id"], "art-1");
}
