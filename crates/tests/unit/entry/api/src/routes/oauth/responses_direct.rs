//! Direct tests for OAuth response helper functions
//!
//! Tests the response helpers now accessible at `systemprompt_api::routes::oauth::responses`:
//! `error_response`, `internal_error`, `not_found`, `bad_request`, `single_response`,
//! `init_error`, and `created_response`.

use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::oauth::responses::{
    bad_request, created_response, error_response, init_error, internal_error, not_found,
    single_response,
};

// ============================================================================
// error_response Tests
// ============================================================================

#[tokio::test]
async fn error_response_returns_correct_status_code() {
    let response = error_response(StatusCode::FORBIDDEN, "access_denied", "Not allowed");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn error_response_body_contains_error_and_description() {
    let response = error_response(
        StatusCode::BAD_REQUEST,
        "invalid_grant",
        "The authorization code has expired",
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "invalid_grant");
    assert_eq!(
        json["error_description"],
        "The authorization code has expired"
    );
}

// ============================================================================
// internal_error Tests
// ============================================================================

#[tokio::test]
async fn internal_error_returns_500() {
    let response = internal_error("Database connection failed");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "server_error");
    assert_eq!(json["error_description"], "Database connection failed");
}

// ============================================================================
// not_found Tests
// ============================================================================

#[tokio::test]
async fn not_found_returns_404() {
    let response = not_found("Client not found");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "not_found");
    assert_eq!(json["error_description"], "Client not found");
}

// ============================================================================
// bad_request Tests
// ============================================================================

#[tokio::test]
async fn bad_request_returns_400() {
    let response = bad_request("Missing required parameter: redirect_uri");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "bad_request");
    assert_eq!(
        json["error_description"],
        "Missing required parameter: redirect_uri"
    );
}

// ============================================================================
// single_response Tests
// ============================================================================

#[tokio::test]
async fn single_response_wraps_data_in_data_field() {
    let data = serde_json::json!({
        "name": "test_client",
        "count": 42
    });

    let response = single_response(data);

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["data"]["name"], "test_client");
    assert_eq!(json["data"]["count"], 42);
}

// ============================================================================
// init_error Tests
// ============================================================================

#[tokio::test]
async fn init_error_includes_formatted_message() {
    let response = init_error("connection refused");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "server_error");
    assert_eq!(
        json["error_description"],
        "Repository initialization failed: connection refused"
    );
}

// ============================================================================
// created_response Tests
// ============================================================================

#[tokio::test]
async fn created_response_returns_201_with_location_header() {
    let body_value = serde_json::json!({"id": "client_123", "name": "New Client"});
    let location = "https://example.com/clients/client_123".to_string();

    let response = created_response(body_value.clone(), location);

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "https://example.com/clients/client_123"
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["id"], "client_123");
    assert_eq!(json["name"], "New Client");
}
