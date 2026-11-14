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
use std::sync::Arc;

use axum::Extension;
use systemprompt_api::services::server::{ReadinessState, SchedulerHealth, discovery_router};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use tower::ServiceExt;

async fn ctx() -> AppContext {
    let url = test_database_url();
    let pool = test_db_pool().await;
    (*test_app_context(&pool, &url)).clone()
}

async fn probe(
    ctx: &AppContext,
    readiness: &Arc<ReadinessState>,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let response = discovery_router(ctx)
        .expect("discovery router builds")
        .layer(Extension(Arc::clone(readiness)))
        .layer(Extension(Arc::new(SchedulerHealth::new())))
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
    let readiness = Arc::new(ReadinessState::new());
    let expected = ctx.config().instance_id.as_str().to_owned();

    let (status, body) = probe(&ctx, &readiness, "/livez").await;

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
    let readiness = Arc::new(ReadinessState::new());

    let (status, body) = probe(&ctx, &readiness, "/readyz").await;

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
    let readiness = Arc::new(ReadinessState::new());
    readiness.signal_ready();

    let (status, body) = probe(&ctx, &readiness, "/readyz").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ready");
    assert_eq!(
        body["instance"],
        ctx.config().instance_id.as_str(),
        "{body}"
    );
}

#[tokio::test]
async fn readyz_stops_admitting_the_moment_the_drain_starts() {
    let ctx = ctx().await;
    let readiness = Arc::new(ReadinessState::new());
    readiness.signal_ready();
    let (ready, _) = probe(&ctx, &readiness, "/readyz").await;
    assert_eq!(ready, StatusCode::OK, "precondition: the node was admitted");

    readiness.signal_shutdown();

    let (status, body) = probe(&ctx, &readiness, "/readyz").await;
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
    let readiness = Arc::new(ReadinessState::new());
    readiness.signal_ready();
    readiness.signal_shutdown();

    let (status, body) = probe(&ctx, &readiness, "/livez").await;

    assert_eq!(
        status,
        StatusCode::OK,
        "liveness is not readiness: killing a draining node would abort its in-flight \
         requests"
    );
    assert_eq!(body["status"], "alive");
}

#[tokio::test]
async fn probes_and_health_report_the_build_the_embedding_binary_named() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let build =
        systemprompt_models::BuildIdentity::new("acme-platform", "3.4.5").with_revision("0123abcd");
    let ctx = (*systemprompt_test_fixtures::app_context::fixture_app_context_with_build(
        &pool, &url, build,
    )
    .expect("fixture context"))
    .clone();
    let readiness = Arc::new(ReadinessState::new());

    for path in ["/livez", "/readyz", "/api/v1/health"] {
        let (_, body) = probe(&ctx, &readiness, path).await;
        assert_eq!(body["build"]["name"], "acme-platform", "{path}: {body}");
        assert_eq!(body["build"]["version"], "3.4.5", "{path}: {body}");
        assert_eq!(body["build"]["revision"], "0123abcd", "{path}: {body}");
        assert!(
            body["version"].as_str().is_some_and(|v| !v.is_empty()),
            "{path}: core's version stays alongside the build: {body}"
        );
    }
}

#[tokio::test]
async fn a_binary_that_names_no_build_reports_core_and_no_revision() {
    let ctx = ctx().await;
    let readiness = Arc::new(ReadinessState::new());

    let (_, body) = probe(&ctx, &readiness, "/livez").await;

    assert_eq!(body["build"]["name"], "systemprompt", "{body}");
    assert_eq!(body["build"]["version"], body["version"], "{body}");
    assert!(body["build"].get("revision").is_none(), "{body}");
}
