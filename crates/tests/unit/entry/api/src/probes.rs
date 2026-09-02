//! `/livez` and `/readyz`, the balancer-facing probes added in 0.44.
//!
//! Liveness and readiness are deliberately different signals: `/livez` says
//! the process is alive and must keep answering while the node drains, so an
//! orchestrator does not kill a machine that is finishing in-flight work;
//! `/readyz` is the admission signal and must refuse before boot completes and
//! from the moment the drain starts.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::services::server::{discovery_router, signal_ready, signal_shutdown};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use tower::ServiceExt;

async fn ctx() -> AppContext {
    let url = fixture_database_url().expect("DATABASE_URL must be set for the probe tests");
    let pool = fixture_db_pool(&url).await.expect("fixture pool");
    (*fixture_app_context(&pool, &url).expect("fixture AppContext")).clone()
}

async fn probe(ctx: &AppContext, path: &str) -> (StatusCode, serde_json::Value) {
    let response = discovery_router(ctx)
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

#[tokio::test]
async fn livez_answers_alive_and_names_the_replica_that_answered() {
    let ctx = ctx().await;
    let expected = ctx.config().instance_id.clone();

    let (status, body) = probe(&ctx, "/livez").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "alive");
    assert_eq!(
        body["instance"], expected,
        "a probe that cannot be attributed to a replica is useless in a multi-replica \
         deployment"
    );
    assert!(
        body["version"].as_str().is_some_and(|v| !v.is_empty()),
        "{body}"
    );
}

#[tokio::test]
async fn readyz_refuses_admission_before_the_process_signals_ready() {
    let ctx = ctx().await;

    let (status, body) = probe(&ctx, "/readyz").await;

    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "a node that never signalled ready must not be admitted: {body}"
    );
    assert_eq!(body["status"], "draining");
}

#[tokio::test]
async fn readyz_admits_the_node_once_it_is_ready_and_the_database_answers() {
    let ctx = ctx().await;
    signal_ready();

    let (status, body) = probe(&ctx, "/readyz").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ready");
    assert_eq!(body["instance"], ctx.config().instance_id, "{body}");
}

#[tokio::test]
async fn readyz_stops_admitting_the_moment_the_drain_starts() {
    let ctx = ctx().await;
    signal_ready();
    let (ready, _) = probe(&ctx, "/readyz").await;
    assert_eq!(ready, StatusCode::OK, "precondition: the node was admitted");

    signal_shutdown();

    let (status, body) = probe(&ctx, "/readyz").await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "the balancer must stop sending new work as soon as the drain begins"
    );
    assert_eq!(body["status"], "draining");
}

#[tokio::test]
async fn livez_keeps_answering_alive_while_the_node_drains() {
    let ctx = ctx().await;
    signal_ready();
    signal_shutdown();

    let (status, body) = probe(&ctx, "/livez").await;

    assert_eq!(
        status,
        StatusCode::OK,
        "liveness is not readiness: killing a draining node would abort its in-flight \
         requests"
    );
    assert_eq!(body["status"], "alive");
}
