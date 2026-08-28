//! Plain-body proxy responses and forwarded-request stat counters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Response, StatusCode};

use crate::proxy::forward::ProxyBody;
use crate::proxy::server::ProxyStats;

pub(super) fn record_stats(stats: &ProxyStats, status: u16, latency_ms: u64) {
    stats.forwarded_total.fetch_add(1, Ordering::Relaxed);
    stats
        .last_forwarded_at_unix
        .store(now_unix(), Ordering::Relaxed);
    stats
        .last_status
        .store(u64::from(status), Ordering::Relaxed);
    stats.last_latency_ms.store(latency_ms, Ordering::Relaxed);
}

pub(super) fn simple_response(status: StatusCode, body: &'static str) -> Response<ProxyBody> {
    let full = Full::new(Bytes::from_static(body.as_bytes()))
        .map_err(|never| match never {})
        .boxed();
    let mut resp = Response::new(full);
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain"),
    );
    resp.headers_mut().insert(
        http::header::CONNECTION,
        http::HeaderValue::from_static("close"),
    );
    resp
}

pub(super) fn owned_response(status: StatusCode, body: String) -> Response<ProxyBody> {
    let full = Full::new(Bytes::from(body))
        .map_err(|never| match never {})
        .boxed();
    let mut resp = Response::new(full);
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain"),
    );
    resp.headers_mut().insert(
        http::header::CONNECTION,
        http::HeaderValue::from_static("close"),
    );
    resp
}

// Why: `owned_response` fixes `text/plain`, which several 4xx paths rely on, so
// JSON gets its own constructor rather than a mutable content type.
pub(super) fn json_response(status: StatusCode, body: String) -> Response<ProxyBody> {
    let mut resp = owned_response(status, body);
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    resp
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
