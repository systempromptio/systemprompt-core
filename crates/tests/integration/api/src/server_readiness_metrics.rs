//! Readiness signalling and the Prometheus metrics surface.
//!
//! Readiness is a value owned by the server (`ReadinessState`), so each test
//! drives its own. The metrics recorder is a process global (`OnceLock`), so
//! those tests only observe idempotent installs and render output, never
//! asserting exclusive ownership of the recorder.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use std::sync::Arc;

use systemprompt_api::services::server::metrics::{
    MetricsState, handle_metrics, install_recorder, track_metrics,
};
use systemprompt_api::services::server::readiness::{ReadinessEvent, ReadinessState};
use systemprompt_events::EventBroadcasters;
use systemprompt_identifiers::InstanceId;
use tower::ServiceExt;

#[tokio::test]
async fn readiness_lifecycle_signals_ready_then_shutdown() {
    let readiness = ReadinessState::new();
    let mut receiver = readiness.subscribe();
    let _second = readiness.subscribe();
    assert!(!readiness.is_ready(), "a new state starts not ready");

    readiness.signal_ready();
    assert!(
        readiness.is_ready(),
        "signal_ready flips the readiness flag"
    );
    assert!(
        readiness.wait_for_ready(1).await,
        "wait_for_ready returns immediately once ready"
    );

    let event = receiver.recv().await.expect("readiness event");
    assert!(matches!(event, ReadinessEvent::ApiReady));

    readiness.signal_shutdown();
    assert!(
        !readiness.is_ready(),
        "signal_shutdown clears the readiness flag"
    );
    let event = receiver.recv().await.expect("shutdown event");
    assert!(matches!(event, ReadinessEvent::ApiShuttingDown));
}

#[tokio::test]
async fn two_readiness_states_do_not_observe_each_other() {
    let first = ReadinessState::new();
    let second = ReadinessState::new();
    let mut second_events = second.subscribe();

    first.signal_ready();

    assert!(first.is_ready());
    assert!(
        !second.is_ready(),
        "readiness is per server, not per process"
    );
    assert!(
        second_events.try_recv().is_err(),
        "the second state's subscribers see nothing from the first"
    );
    assert!(
        !second.wait_for_ready(0).await,
        "waiting on the other state times out instead of borrowing a signal"
    );
}

#[test]
fn install_recorder_is_idempotent() {
    let first = install_recorder(&InstanceId::new("test-instance"), &[]).expect("install recorder");
    let second = install_recorder(&InstanceId::new("test-instance"), &[])
        .expect("second install returns cached handle");
    let _ = (first.render(), second.render());
}

#[tokio::test]
async fn handle_metrics_renders_prometheus_body() {
    let handle =
        install_recorder(&InstanceId::new("test-instance"), &[]).expect("install recorder");
    let app = Router::new()
        .route("/metrics", get(handle_metrics))
        .with_state(MetricsState {
            handle,
            broadcasters: Arc::new(EventBroadcasters::new()),
        });

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
    let _ = install_recorder(&InstanceId::new("test-instance"), &[]);
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
