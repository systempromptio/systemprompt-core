//! Readiness signalling and the Prometheus metrics surface.
//!
//! Both modules lean on process globals (a static `AtomicBool`/broadcast for
//! readiness, a `OnceLock` recorder for metrics), so the readiness lifecycle is
//! driven inside a single test to keep the global toggles deterministic under
//! the parallel runner; the metrics tests only observe idempotent installs and
//! render output, never asserting exclusive ownership of the recorder.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_api::services::server::metrics::{
    handle_metrics, install_recorder, track_metrics,
};
use systemprompt_api::services::server::readiness::{
    ReadinessEvent, get_readiness_receiver, init_readiness, is_ready, signal_ready,
    signal_shutdown, wait_for_ready,
};
use tower::ServiceExt;

#[tokio::test]
async fn readiness_lifecycle_signals_ready_then_shutdown() {
    let mut receiver = init_readiness();
    let _second = get_readiness_receiver();

    signal_ready();
    assert!(is_ready(), "signal_ready flips the readiness flag");
    assert!(
        wait_for_ready(1).await,
        "wait_for_ready returns immediately once ready"
    );

    let event = receiver.recv().await.expect("readiness event");
    assert!(matches!(event, ReadinessEvent::ApiReady));

    signal_shutdown();
    assert!(!is_ready(), "signal_shutdown clears the readiness flag");
    let event = receiver.recv().await.expect("shutdown event");
    assert!(matches!(event, ReadinessEvent::ApiShuttingDown));
}

#[test]
fn install_recorder_is_idempotent() {
    let first = install_recorder().expect("install recorder");
    let second = install_recorder().expect("second install returns cached handle");
    let _ = (first.render(), second.render());
}

#[tokio::test]
async fn handle_metrics_renders_prometheus_body() {
    let handle = install_recorder().expect("install recorder");
    let app = Router::new()
        .route("/metrics", get(handle_metrics))
        .with_state(handle);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("content-type")
            .to_str()
            .expect("utf8")
            .starts_with("text/plain")
    );
    let _body = to_bytes(resp.into_body(), 1 << 20).await.expect("body");
}

#[tokio::test]
async fn track_metrics_middleware_records_request_and_forwards() {
    let _ = install_recorder();
    let app = Router::new()
        .route("/ok", get(|| async { "ok" }))
        .route("/boom", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
        .layer(axum::middleware::from_fn(track_metrics));

    let ok = app
        .clone()
        .oneshot(Request::builder().uri("/ok").body(Body::empty()).unwrap())
        .await
        .expect("ok response");
    assert_eq!(ok.status(), StatusCode::OK);

    let boom = app
        .oneshot(Request::builder().uri("/boom").body(Body::empty()).unwrap())
        .await
        .expect("boom response");
    assert_eq!(boom.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
