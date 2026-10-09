//! Prometheus recorder installation and HTTP metrics rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::header::CONTENT_TYPE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, Broadcaster, CONTEXT_BROADCASTER,
};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::OwnedTask;

const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
pub const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";
pub const HTTP_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];
const HTTP_REQUESTS_IN_FLIGHT: &str = "http_requests_in_flight";
const SSE_CONNECTIONS: &str = "sse_active_connections";

// Why: metrics::set_global_recorder rejects a second installation in the same
// process.
static RECORDER: OnceLock<PrometheusHandle> = OnceLock::new();
static RECORDER_INIT: Mutex<()> = Mutex::new(());

pub fn install_recorder(instance_id: &InstanceId) -> anyhow::Result<PrometheusHandle> {
    if let Some(handle) = RECORDER.get() {
        return Ok(handle.clone());
    }
    let _guard = RECORDER_INIT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(handle) = RECORDER.get() {
        return Ok(handle.clone());
    }
    let handle = PrometheusBuilder::new()
        .add_global_label("instance", instance_id.as_str())
        .set_buckets_for_metric(
            Matcher::Full(systemprompt_gateway::GATEWAY_OVERHEAD_SECONDS.to_owned()),
            &systemprompt_gateway::OVERHEAD_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(systemprompt_gateway::GATEWAY_UPSTREAM_DURATION_SECONDS.to_owned()),
            &systemprompt_gateway::UPSTREAM_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(HTTP_REQUEST_DURATION_SECONDS.to_owned()),
            &HTTP_BUCKETS,
        )?
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus recorder: {e}"))?;
    describe_metrics();
    Ok(RECORDER.get_or_init(|| handle).clone())
}

fn describe_metrics() {
    super::pool_metrics::describe();
    use crate::services::middleware::load_shed::{
        HTTP_IN_FLIGHT_LIMIT, HTTP_IN_FLIGHT_SATURATION, HTTP_LOAD_SHED_TOTAL,
    };
    metrics::describe_counter!(
        HTTP_LOAD_SHED_TOTAL,
        "Requests refused with 503 because the in-flight ceiling was reached"
    );
    metrics::describe_gauge!(
        HTTP_IN_FLIGHT_LIMIT,
        "Configured server.max_in_flight ceiling"
    );
    metrics::describe_gauge!(
        HTTP_IN_FLIGHT_SATURATION,
        "Fraction of the in-flight ceiling in use (1.0 = shedding)"
    );
    metrics::describe_histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        metrics::Unit::Seconds,
        "HTTP request duration by method, matched path and status"
    );
    metrics::describe_histogram!(
        systemprompt_gateway::GATEWAY_OVERHEAD_SECONDS,
        metrics::Unit::Seconds,
        "Time the gateway added to a completed inference request, excluding the upstream call"
    );
    metrics::describe_histogram!(
        systemprompt_gateway::GATEWAY_UPSTREAM_DURATION_SECONDS,
        metrics::Unit::Seconds,
        "Upstream provider call duration for a completed inference request"
    );
}

pub fn metrics_router(handle: PrometheusHandle) -> axum::Router {
    axum::Router::new()
        .route("/metrics", axum::routing::get(handle_metrics))
        .with_state(handle)
}

pub async fn serve_metrics_listener(
    addr: std::net::SocketAddr,
    handle: PrometheusHandle,
) -> anyhow::Result<OwnedTask<()>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let mut readiness = super::readiness::get_readiness_receiver();
    let shutdown = async move {
        loop {
            match readiness.recv().await {
                Ok(super::readiness::ReadinessEvent::ApiShuttingDown) | Err(_) => break,
                Ok(_) => {},
            }
        }
    };
    tracing::info!(%addr, "metrics listener bound");
    Ok(OwnedTask::spawn("metrics_listener", async move {
        if let Err(error) = axum::serve(listener, metrics_router(handle))
            .with_graceful_shutdown(shutdown)
            .await
        {
            tracing::warn!(error = %error, "metrics listener exited with error");
        }
    }))
}

pub async fn handle_metrics(
    axum::extract::State(handle): axum::extract::State<PrometheusHandle>,
) -> Response {
    refresh_connection_gauges().await;
    let body = handle.render();
    ([(CONTENT_TYPE, METRICS_CONTENT_TYPE)], body).into_response()
}

async fn refresh_connection_gauges() {
    let context = CONTEXT_BROADCASTER.total_connections().await;
    let agui = AGUI_BROADCASTER.total_connections().await;
    let a2a = A2A_BROADCASTER.total_connections().await;
    let analytics = ANALYTICS_BROADCASTER.total_connections().await;

    metrics::gauge!(SSE_CONNECTIONS, "channel" => "context").set(context as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "agui").set(agui as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "a2a").set(a2a as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "analytics").set(analytics as f64);
}

pub async fn track_metrics(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| req.uri().path().to_owned(), |m| m.as_str().to_owned());

    let in_flight = metrics::gauge!(HTTP_REQUESTS_IN_FLIGHT);
    in_flight.increment(1.0);

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();

    in_flight.decrement(1.0);

    let status = response.status().as_u16().to_string();
    let method = method.to_string();

    metrics::counter!(
        HTTP_REQUESTS_TOTAL,
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .increment(1);
    metrics::histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .record(elapsed);

    response
}
