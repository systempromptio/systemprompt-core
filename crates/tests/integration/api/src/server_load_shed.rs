//! Overload shedding on a router carrying the real probes and `/metrics`.
//!
//! With a ceiling of one in-flight request, a second request is refused at
//! once with `503`, `Retry-After: 1` and the `overloaded` error key, while the
//! probes and the scrape endpoint keep answering and `/readyz` reports the
//! replica saturated. Releasing the held request restores admission. A
//! streamed response holds its permit until its body is dropped, not only
//! until its headers are sent.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures::SinkExt as _;
use futures::channel::mpsc;
use systemprompt_api::services::middleware::load_shed::outermost;
use systemprompt_api::services::server::metrics::{MetricsState, install_recorder, metrics_router};
use systemprompt_api::services::server::{ReadinessState, discovery_router};
use systemprompt_identifiers::InstanceId;
use systemprompt_test_fixtures::{test_app_context, test_database_url};
use tokio::sync::Notify;
use tower::ServiceExt;

type Chunks = mpsc::Receiver<Result<bytes::Bytes, std::io::Error>>;

async fn app(
    release: Arc<Notify>,
    entered: Arc<Notify>,
    streams: Arc<tokio::sync::Mutex<Option<Chunks>>>,
) -> Router {
    let url = test_database_url();
    let ctx = (*test_app_context(&url).await).clone();
    let handle = install_recorder(&InstanceId::new("load-shed-fixture"), &[]).expect("recorder");
    let slow = axum::routing::get(move || {
        let release = Arc::clone(&release);
        let entered = Arc::clone(&entered);
        async move {
            entered.notify_one();
            release.notified().await;
            "done"
        }
    });
    let stream = axum::routing::get(move || {
        let streams = Arc::clone(&streams);
        async move {
            let chunks = streams.lock().await.take().expect("one stream per fixture");
            Body::from_stream(chunks)
        }
    });
    let readiness = Arc::new(ReadinessState::new());
    readiness.signal_ready();
    let router = discovery_router(&ctx)
        .expect("discovery router builds")
        .layer(axum::Extension(readiness))
        .merge(metrics_router(MetricsState {
            handle,
            broadcasters: Arc::clone(ctx.event_broadcasters()),
        }))
        .route("/slow", slow)
        .route("/stream", stream);
    outermost(router, NonZeroU32::new(1))
}

async fn in_flight(app: &Router) -> serde_json::Value {
    let (_, _, body, _) = get(app, "/readyz").await;
    body
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Option<String>, serde_json::Value, String) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = resp.status();
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(resp.into_body(), 256 * 1024)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let json = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    (status, retry_after, json, text)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saturated_replica_sheds_requests_but_keeps_answering_probes() {
    let release = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let app = app(
        Arc::clone(&release),
        Arc::clone(&entered),
        Arc::new(tokio::sync::Mutex::new(None)),
    )
    .await;

    assert_eq!(in_flight(&app).await["status"], "ready");
    let held = tokio::spawn({
        let app = app.clone();
        async move { get(&app, "/slow").await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the first request reaches the handler");

    let (status, retry_after, body, _) = get(&app, "/slow").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(retry_after.as_deref(), Some("1"));
    assert_eq!(body["error_key"], "overloaded", "{body}");

    assert_eq!(get(&app, "/livez").await.0, StatusCode::OK);
    let (ready, _, ready_body, _) = get(&app, "/readyz").await;
    assert_eq!(ready, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ready_body["status"], "saturated", "{ready_body}");
    assert_eq!(ready_body["in_flight"], 1);
    assert_eq!(ready_body["limit"], 1);
    let (scrape, _, _, metrics) = get(&app, "/metrics").await;
    assert_eq!(scrape, StatusCode::OK);
    assert!(metrics.contains("http_load_shed_total"), "{metrics}");
    assert!(metrics.contains("http_in_flight_saturation"), "{metrics}");

    release.notify_one();
    assert_eq!(held.await.expect("held request"), StatusCode::OK);
    assert_eq!(in_flight(&app).await["status"], "ready");

    let next = tokio::spawn({
        let app = app.clone();
        async move { get(&app, "/slow").await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("admission is restored");
    release.notify_one();
    assert_eq!(next.await.expect("next request"), StatusCode::OK);
    assert_eq!(get(&app, "/readyz").await.0, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_streamed_response_holds_its_permit_until_the_body_is_dropped() {
    let (mut sender, chunks) = mpsc::channel(4);
    let app = app(
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
        Arc::new(tokio::sync::Mutex::new(Some(chunks))),
    )
    .await;

    let streamed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/stream")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response head");
    assert_eq!(streamed.status(), StatusCode::OK);
    sender
        .send(Ok(bytes::Bytes::from_static(b"data: first\n\n")))
        .await
        .expect("stream open");

    let (status, retry_after, body, _) = get(&app, "/slow").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(retry_after.as_deref(), Some("1"));
    let ready = in_flight(&app).await;
    assert_eq!(ready["status"], "saturated", "{ready}");
    assert_eq!(ready["in_flight"], 1);

    drop(streamed);
    drop(sender);
    assert_eq!(in_flight(&app).await["status"], "ready");
}
