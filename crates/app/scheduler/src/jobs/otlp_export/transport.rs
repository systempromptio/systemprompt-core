//! OTLP/HTTP delivery: protobuf POST with bounded retry and the
//! `otlp_export_batches_total{signal,status}` counter.
//!
//! A batch gets [`RETRY_DELAYS`] retries in-run, each after the listed
//! pause. One that fails every attempt keeps its watermark and is retried at
//! the next tick, so the in-run budget only needs to ride out a blip, not an
//! outage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use prost::Message;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use systemprompt_models::profile::{OtlpExportConfig, OtlpSignal};

pub const BATCHES_TOTAL: &str = "otlp_export_batches_total";
const CONTENT_TYPE_PROTOBUF: &str = "application/x-protobuf";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(500),
    Duration::from_millis(1500),
    Duration::from_millis(4000),
];

// Why: the OTLP/HTTP spec names 429, 502, 503 and 504 as the retryable
// responses; every other 4xx is a request the collector will keep refusing.
#[must_use]
pub fn is_retryable(status: Option<StatusCode>) -> bool {
    status.is_none_or(|status| {
        matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
        )
    })
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TransportError {
    #[error("invalid header {name}: {reason}")]
    Header { name: String, reason: String },
    #[error("collector answered {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(
        || match reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(error = %error, "OTLP client builder failed; using default client");
                reqwest::Client::new()
            },
        },
    )
}

pub(super) fn build_headers(config: &OtlpExportConfig) -> Result<HeaderMap, TransportError> {
    let mut headers = HeaderMap::with_capacity(config.headers.len() + 1);
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static(CONTENT_TYPE_PROTOBUF),
    );
    for (name, value) in &config.headers {
        let key = HeaderName::from_bytes(name.as_bytes()).map_err(|e| TransportError::Header {
            name: name.clone(),
            reason: e.to_string(),
        })?;
        let value = HeaderValue::from_str(value).map_err(|e| TransportError::Header {
            name: name.clone(),
            reason: e.to_string(),
        })?;
        headers.insert(key, value);
    }
    Ok(headers)
}

pub(super) async fn post_signal<M: Message>(
    config: &OtlpExportConfig,
    signal: OtlpSignal,
    envelope: &M,
) -> Result<(), TransportError> {
    let url = config.signal_url(signal);
    let headers = build_headers(config)?;
    let body = envelope.encode_to_vec();

    let mut delays = RETRY_DELAYS.iter();
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let error = match send_once(&url, headers.clone(), body.clone()).await {
            Ok(()) => {
                record(signal, "ok");
                return Ok(());
            },
            Err(error) => error,
        };
        let retry = match &error {
            TransportError::Status { status, .. } => is_retryable(Some(*status)),
            TransportError::Request(e) => is_retryable(e.status()),
            TransportError::Header { .. } => false,
        };
        tracing::warn!(
            signal = %signal, attempt, retry, error = %error,
            "OTLP export attempt failed"
        );
        match delays.next() {
            Some(delay) if retry => tokio::time::sleep(*delay).await,
            _ => {
                record(signal, "error");
                return Err(error);
            },
        }
    }
}

async fn send_once(url: &str, headers: HeaderMap, body: Vec<u8>) -> Result<(), TransportError> {
    let response = http_client()
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = match response.text().await {
        Ok(text) => text.chars().take(512).collect(),
        Err(error) => format!("<body unreadable: {error}>"),
    };
    Err(TransportError::Status { status, body })
}

fn record(signal: OtlpSignal, status: &'static str) {
    metrics::counter!(BATCHES_TOTAL, "signal" => signal.label(), "status" => status).increment(1);
}
