//! Admission is decided outside every other layer: with the one in-flight
//! permit held, a request that the inner authentication layer would refuse
//! with `401` is answered `503` with `Retry-After` instead, so a saturated
//! replica spends nothing on authentication or body parsing. An explicit
//! `None` ceiling mounts no admission layer.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_api::services::middleware::load_shed::{InFlightLimits, outermost};
use tokio::sync::Notify;
use tower::ServiceExt;

async fn refuse_unauthenticated(req: Request, next: Next) -> Response {
    if req.headers().contains_key(header::AUTHORIZATION) {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

fn app(ceiling: Option<NonZeroU32>, release: Arc<Notify>, entered: Arc<Notify>) -> Router {
    let slow = axum::routing::get(move || {
        let release = Arc::clone(&release);
        let entered = Arc::clone(&entered);
        async move {
            entered.notify_one();
            release.notified().await;
            "done"
        }
    });
    let inner = Router::new()
        .route("/slow", slow)
        .route("/private", axum::routing::post(|| async { "private" }))
        .layer(axum::middleware::from_fn(refuse_unauthenticated))
        .layer(axum::extract::DefaultBodyLimit::max(16));
    outermost(inner, limits(ceiling))
}

fn limits(requests: Option<NonZeroU32>) -> InFlightLimits {
    InFlightLimits {
        requests,
        probes: systemprompt_manifest::profile::DEFAULT_MAX_PROBE_IN_FLIGHT,
    }
}

async fn send(app: &Router, method: &str, uri: &str, auth: bool) -> (StatusCode, Option<String>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if auth {
        builder = builder.header(header::AUTHORIZATION, "Bearer fixture");
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(vec![b'x'; 64])).expect("request"))
        .await
        .expect("response");
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    (response.status(), retry_after)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saturated_replica_answers_503_before_authentication() {
    let release = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let app = app(
        NonZeroU32::new(1),
        Arc::clone(&release),
        Arc::clone(&entered),
    );

    assert_eq!(
        send(&app, "POST", "/private", false).await.0,
        StatusCode::UNAUTHORIZED
    );

    let held = tokio::spawn({
        let app = app.clone();
        async move { send(&app, "GET", "/slow", true).await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the held request reaches its handler");

    let (status, retry_after) = send(&app, "POST", "/private", false).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(retry_after.as_deref(), Some("1"));

    release.notify_one();
    assert_eq!(held.await.expect("held request"), StatusCode::OK);
    assert_eq!(
        send(&app, "POST", "/private", false).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn no_ceiling_mounts_no_admission_layer() {
    let app = app(None, Arc::new(Notify::new()), Arc::new(Notify::new()));
    assert_eq!(
        send(&app, "POST", "/private", false).await.0,
        StatusCode::UNAUTHORIZED
    );
}

async fn p99_under_load(ceiling: Option<NonZeroU32>, rps: u32, seconds: u32) -> (Duration, usize) {
    let handler = axum::routing::get(|| async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "ok"
    });
    let app = outermost(Router::new().route("/work", handler), limits(ceiling));
    let interval = Duration::from_secs(1) / rps;
    let mut tasks = Vec::new();
    let mut ticker = tokio::time::interval(interval);
    for _ in 0..(rps * seconds) {
        ticker.tick().await;
        let app = app.clone();
        tasks.push(tokio::spawn(async move {
            let started = std::time::Instant::now();
            let status = app
                .oneshot(Request::get("/work").body(Body::empty()).expect("request"))
                .await
                .expect("response")
                .status();
            (started.elapsed(), status)
        }));
    }
    let mut latencies = Vec::new();
    let mut shed = 0;
    for task in tasks {
        let (latency, status) = task.await.expect("request task");
        if status == StatusCode::SERVICE_UNAVAILABLE {
            shed += 1;
        }
        latencies.push(latency);
    }
    latencies.sort_unstable();
    (latencies[latencies.len() * 99 / 100], shed)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load measurement of admission control; run on demand"]
async fn p99_with_and_without_admission_at_rising_rates() {
    for rps in [500, 1000, 2000, 4000] {
        let (open, _) = p99_under_load(None, rps, 5).await;
        let (bounded, shed) = p99_under_load(NonZeroU32::new(64), rps, 5).await;
        println!(
            "load_shed rps={rps} p99_unbounded_ms={} p99_admitted_ms={} shed={shed}",
            open.as_millis(),
            bounded.as_millis()
        );
    }
}
