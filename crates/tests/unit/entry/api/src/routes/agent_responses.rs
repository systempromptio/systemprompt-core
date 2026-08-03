//! Agent route response constructors.
//!
//! Four `pub` builders that no test calls. They decide the status code and the
//! envelope shape every agent route returns, so a change to either is a wire
//! break for A2A clients.

use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::agent::responses::{
    api_error_response, collection_response, single_response, single_response_created,
};
use systemprompt_models::ApiError;

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("response bodies here are small and fully buffered");
    serde_json::from_slice(&bytes).expect("every agent response is JSON")
}

#[tokio::test]
async fn a_single_response_is_a_200_wrapping_its_payload() {
    let resp = single_response(serde_json::json!({"id": "task-1"}));
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(
        body["data"]["id"].as_str(),
        Some("task-1"),
        "the payload is nested under `data`, not returned bare: {body}"
    );
}

#[tokio::test]
async fn a_created_response_differs_only_in_status() {
    let created = single_response_created(serde_json::json!({"id": "task-2"}));
    assert_eq!(created.status(), StatusCode::CREATED);

    let body = body_json(created).await;
    assert_eq!(body["data"]["id"].as_str(), Some("task-2"), "{body}");
}

#[tokio::test]
async fn a_collection_response_wraps_its_items() {
    let resp = collection_response(vec![
        serde_json::json!({"id": "a"}),
        serde_json::json!({"id": "b"}),
    ]);
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    let items = body["data"]
        .as_array()
        .expect("a collection nests its items under `data`");
    assert_eq!(items.len(), 2, "{body}");
    assert_eq!(items[0]["id"].as_str(), Some("a"), "{body}");
}

#[tokio::test]
async fn an_empty_collection_is_an_empty_array_not_null() {
    let resp = collection_response(Vec::<serde_json::Value>::new());

    let body = body_json(resp).await;
    assert_eq!(
        body["data"].as_array().map(Vec::len),
        Some(0),
        "an empty result must stay an array so clients can iterate it: {body}"
    );
}

#[tokio::test]
async fn an_error_response_takes_its_status_from_the_error_code() {
    for (error, expected) in [
        (ApiError::not_found("gone"), StatusCode::NOT_FOUND),
        (ApiError::bad_request("nope"), StatusCode::BAD_REQUEST),
        (
            ApiError::internal_error("boom"),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ] {
        let message = error.message.clone();
        let resp = api_error_response(error);
        assert_eq!(resp.status(), expected, "{message}");
    }
}

#[tokio::test]
async fn an_error_response_carries_the_message_to_the_client() {
    let resp = api_error_response(ApiError::not_found("agent 'ghost' not found"));

    let body = body_json(resp).await;
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("ghost")),
        "the caller must be told which agent was missing: {body}"
    );
}
