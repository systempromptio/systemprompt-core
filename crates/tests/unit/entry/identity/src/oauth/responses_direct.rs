//! Direct tests for the OAuth response helpers retained after the
//! OAuthHttpError migration: `single_response`, `created_response`.

use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::oauth::responses::{created_response, single_response};

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
