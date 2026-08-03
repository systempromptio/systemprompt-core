//! The OAuth response builders in `routes::oauth::responses`.
//!
//! Both are `pub` and neither is called by any test. They fix the envelope and
//! the `Location` header that dynamic client registration returns, which RFC
//! 7591 clients follow to manage the client they just created.

use axum::body::to_bytes;
use axum::http::{StatusCode, header};
use systemprompt_api::routes::oauth::responses::{created_response, single_response};

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("response bodies here are small");
    serde_json::from_slice(&bytes).expect("every oauth response is JSON")
}

#[tokio::test]
async fn a_single_response_nests_its_payload_under_data() {
    let resp = single_response(serde_json::json!({"client_id": "abc"}));
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(body["data"]["client_id"].as_str(), Some("abc"), "{body}");
}

#[tokio::test]
async fn a_created_response_points_at_the_resource_it_created() {
    let location = "/api/v1/core/oauth/register/client_123".to_owned();
    let resp = created_response(
        serde_json::json!({"client_id": "client_123"}),
        location.clone(),
    );

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some(location.as_str()),
        "RFC 7591 clients follow Location to the registration-management URI"
    );
}

#[tokio::test]
async fn a_created_response_returns_the_body_unwrapped() {
    let resp = created_response(
        serde_json::json!({"client_id": "client_9", "client_secret": "s"}),
        "/somewhere".to_owned(),
    );

    let body = body_json(resp).await;
    assert_eq!(
        body["client_id"].as_str(),
        Some("client_9"),
        "registration responses are the bare RFC 7591 object, not a data envelope: {body}"
    );
    assert!(body.get("data").is_none(), "{body}");
}
