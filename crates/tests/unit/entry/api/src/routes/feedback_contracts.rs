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
#[test]
fn openapi_references_resolve_and_consumer_admin_contracts_are_distinct() {
    let document = systemprompt_api::routes::managed::contract::openapi::document();
    assert_eq!(document["openapi"], "3.1.0");
    let paths = document["paths"].as_object().unwrap();
    assert!(
        paths.len() >= 40,
        "the managed surface documents {} paths",
        paths.len()
    );
    fn references(value: &serde_json::Value, root: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(serde_json::Value::as_str)
                    && let Some(pointer) = reference.strip_prefix('#')
                {
                    assert!(root.pointer(pointer).is_some(), "unresolved {reference}");
                }
                for value in map.values() {
                    references(value, root);
                }
            },
            serde_json::Value::Array(values) => {
                for value in values {
                    references(value, root)
                }
            },
            _ => {},
        }
    }
    references(&document, &document);
    let consumer = &paths["/consumer/receipts"]["post"];
    assert!(consumer["security"][0].get("deviceCredential").is_some());
    assert!(
        consumer["requestBody"]["content"]["application/json"]["schema"]
            .get("$ref")
            .is_some()
    );
    assert!(
        paths["/consumer-devices/{id}/credential"]["post"]["security"][0]
            .get("adminBearer")
            .is_some()
    );
    for path in [
        "/inventory/reconciliations",
        "/sources/{id}/captures",
        "/source-verifications",
        "/consumer-devices/{id}/credential",
    ] {
        assert!(
            paths[path]["post"]["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["name"] == "Idempotency-Key" && p["required"] == true)
        );
    }
    assert!(
        paths["/analytics/jobs/{operation}"]["get"]["responses"]
            .get("404")
            .is_some()
    );
    assert!(
        paths["/analytics/live"]["get"]["responses"]["200"]["content"]
            .get("text/event-stream")
            .is_some()
    );
    assert_eq!(
        document["components"]["securitySchemes"]["adminCookie"]["name"],
        "access_token"
    );
}

#[path = "feedback_credential_issuance.rs"]
mod credential_issuance;
