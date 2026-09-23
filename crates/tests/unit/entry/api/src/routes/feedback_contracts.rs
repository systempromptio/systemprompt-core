//! Real HTTP extraction, bounds, problem details and generated schema
//! integrity.
use axum::body::{Body, to_bytes};
use axum::extract::Query;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower::ServiceExt;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    revision_id: String,
}
#[derive(Deserialize)]
struct Number {
    limit: u32,
}
async fn typed(Json(input): Json<Input>) -> Json<String> {
    Json(input.revision_id)
}
async fn query(Query(input): Query<Number>) -> Json<u32> {
    Json(input.limit)
}
fn router() -> Router {
    Router::new()
        .route("/typed", post(typed))
        .route("/query", get(query))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ))
}
async fn check(request: Request<Body>, status: StatusCode) {
    let response = router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), status);
    assert_eq!(
        response.headers()["content-type"],
        "application/problem+json"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["status"], status.as_u16());
    assert!(body["detail"].is_string());
}
#[tokio::test]
async fn invalid_json_query_and_oversized_identifiers_share_problem_contract() {
    check(
        Request::builder()
            .method("POST")
            .uri("/typed")
            .header("content-type", "application/json")
            .body(Body::from("{"))
            .unwrap(),
        StatusCode::BAD_REQUEST,
    )
    .await;
    check(
        Request::builder()
            .method("POST")
            .uri("/typed")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    check(
        Request::builder()
            .uri("/query?limit=nope")
            .body(Body::empty())
            .unwrap(),
        StatusCode::BAD_REQUEST,
    )
    .await;
    check(
        Request::builder()
            .method("POST")
            .uri("/typed")
            .header("content-type", "Application/JSON; charset=utf-8")
            .body(Body::from(
                serde_json::json!({"revision_id":"x".repeat(513)}).to_string(),
            ))
            .unwrap(),
        StatusCode::BAD_REQUEST,
    )
    .await;
    check(
        Request::builder()
            .method("POST")
            .uri("/typed")
            .header("content-type", "application/vendor+json")
            .body(Body::from(
                serde_json::json!({"revision_id":"x".repeat(513)}).to_string(),
            ))
            .unwrap(),
        StatusCode::BAD_REQUEST,
    )
    .await;
    check(
        Request::builder()
            .method("POST")
            .uri("/typed")
            .header("content-type", "application/json")
            .body(Body::from("x".repeat(1024 * 1024 + 1)))
            .unwrap(),
        StatusCode::PAYLOAD_TOO_LARGE,
    )
    .await;
}
