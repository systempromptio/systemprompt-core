//! HTTP plumbing for forwarding a proxied request to a local backend.
//!
//! Splits the forwarding pipeline into focused helpers: [`UrlResolver`] builds
//! the backend URL, [`RequestBuilder`] constructs the outbound `reqwest`
//! request (filtering hop-by-hop and invalid auth headers) and bounds the
//! request body, streaming it upstream rather than buffering it, and
//! [`ResponseHandler`] streams the response back, wrapping SSE bodies in a
//! keep-alive stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_LENGTH;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use futures_util::TryStreamExt;
use http_body_util::{BodyExt, LengthLimitError, Limited};
use reqwest::Method;
use std::str::FromStr;

pub(super) use super::errors::ProxyError;
use super::errors::ResponseBuildError;
pub(super) use super::sse::{SSE_KEEPALIVE_INTERVAL, SseKeepaliveStream};

#[derive(Debug, Clone, Copy)]
pub(super) struct UrlResolver;

impl UrlResolver {
    pub(super) fn build_backend_url(protocol: &str, host: &str, port: i32, path: &str) -> String {
        let clean_path = path.trim_start_matches('/');

        match protocol {
            "mcp" => {
                if clean_path.is_empty() || clean_path == "mcp" {
                    format!("http://{host}:{port}/mcp")
                } else {
                    format!("http://{host}:{port}/{clean_path}")
                }
            },
            _ => {
                if clean_path.is_empty() {
                    format!("http://{host}:{port}/")
                } else {
                    format!("http://{host}:{port}/{clean_path}")
                }
            },
        }
    }

    pub(super) fn append_query_params(url: String, query: Option<&str>) -> String {
        match query {
            Some(q) if !q.is_empty() => format!("{url}?{q}"),
            _ => url,
        }
    }
}

pub(super) const MCP_REQUEST_BODY_LIMIT: usize = 4 * 1024 * 1024;
pub(super) const AGENT_REQUEST_BODY_LIMIT: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct RequestBuilder;

impl RequestBuilder {
    pub(super) fn ensure_declared_length(
        headers: &HeaderMap,
        limit: usize,
    ) -> Result<(), ProxyError> {
        let declared = headers
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        match declared {
            Some(length) if length > limit as u64 => Err(ProxyError::PayloadTooLarge { limit }),
            _ => Ok(()),
        }
    }

    pub(super) async fn read_body(body: Body, limit: usize) -> Result<Vec<u8>, ProxyError> {
        to_bytes(body, limit)
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|source| {
                if is_length_limit(&source) {
                    ProxyError::PayloadTooLarge { limit }
                } else {
                    ProxyError::BodyExtractionFailed { source }
                }
            })
    }

    pub(super) fn streamed_body(body: Body, limit: usize) -> reqwest::Body {
        reqwest::Body::wrap_stream(Limited::new(body, limit).into_data_stream())
    }

    pub(super) fn parse_method(method_str: &str) -> Result<Method, ProxyError> {
        Method::from_str(method_str).map_err(|source| ProxyError::InvalidMethod {
            method: method_str.to_owned(),
            source,
        })
    }

    pub(super) fn build_request(
        client: &reqwest::Client,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Option<reqwest::Body>,
    ) -> reqwest::RequestBuilder {
        let mut req_builder = client.request(method, url);
        req_builder = Self::add_headers(req_builder, headers);
        if let Some(body) = body {
            req_builder = req_builder.body(body);
        }
        req_builder
    }

    pub(super) const fn carries_body(method: &Method) -> bool {
        !matches!(*method, Method::GET | Method::HEAD | Method::DELETE)
    }

    fn add_headers(
        mut req_builder: reqwest::RequestBuilder,
        headers: &HeaderMap,
    ) -> reqwest::RequestBuilder {
        for (key, value) in headers {
            if let Ok(value_str) = value.to_str() {
                let key_str = key.as_str();

                if Self::should_skip_header(key_str) {
                    continue;
                }

                if key_str.eq_ignore_ascii_case("authorization") {
                    if Self::is_valid_auth_header(value_str) {
                        req_builder = req_builder.header(key_str, value_str);
                    }
                } else {
                    req_builder = req_builder.header(key_str, value_str);
                }
            }
        }
        req_builder
    }

    fn should_skip_header(header_name: &str) -> bool {
        let lower_name = header_name.to_lowercase();
        matches!(
            lower_name.as_str(),
            "host" | "x-mcp-proxy-auth" | "content-length" | "transfer-encoding"
        )
    }

    fn is_valid_auth_header(value: &str) -> bool {
        value != "Bearer" && !value.trim().eq_ignore_ascii_case("bearer")
    }
}

pub(super) fn is_length_limit(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(error);
    while let Some(cause) = next {
        if cause.is::<LengthLimitError>() {
            return true;
        }
        next = cause.source();
    }
    false
}

pub(super) fn upstream_status(
    response: &reqwest::Response,
) -> Result<StatusCode, ResponseBuildError> {
    let status = response.status().as_u16();
    StatusCode::from_u16(status).map_err(|source| ResponseBuildError::Status { status, source })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResponseHandler;

impl ResponseHandler {
    pub(super) fn build_response(
        response: reqwest::Response,
    ) -> Result<Response<Body>, ResponseBuildError> {
        let axum_status = upstream_status(&response)?;

        let response_headers = response.headers().clone();
        let is_sse = Self::is_event_stream(&response_headers);

        let stream = response.bytes_stream().map_err(std::io::Error::other);
        let body = if is_sse {
            Body::from_stream(SseKeepaliveStream::new(stream, SSE_KEEPALIVE_INTERVAL))
        } else {
            Body::from_stream(stream)
        };

        Self::assemble(axum_status, &response_headers, is_sse, body)
    }

    pub(super) fn is_event_stream(headers: &HeaderMap) -> bool {
        headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("text/event-stream"))
    }

    pub(super) fn assemble(
        axum_status: StatusCode,
        response_headers: &HeaderMap,
        is_sse: bool,
        body: Body,
    ) -> Result<Response<Body>, ResponseBuildError> {
        let mut axum_response = Response::builder().status(axum_status);

        for (key, value) in response_headers {
            let key_str = key.as_str();
            if let Ok(value_str) = value.to_str()
                && Self::should_preserve_header(key_str)
            {
                axum_response = axum_response.header(key_str, value_str);
            }
        }

        axum_response = axum_response
            .header("connection", "keep-alive")
            .header("cache-control", "no-cache");

        if is_sse {
            axum_response = axum_response
                .header("x-accel-buffering", "no")
                .header("cache-control", "no-cache, no-transform");
        }

        axum_response
            .body(body)
            .map_err(|e| ResponseBuildError::Assemble(Box::new(e)))
    }

    // Why: the audit tap rewrites a buffered JSON body (execution stamp) and
    // reqwest has already decoded any content-encoding, so the upstream framing
    // headers describe a body that no longer exists. Carrying content-length
    // through made hyper panic ("payload claims content-length of N, custom
    // content-length header claims M") on every stamped tools/call response
    // from an external server, which dropped the connection on the client.
    fn should_preserve_header(key: &str) -> bool {
        !matches!(
            key.to_lowercase().as_str(),
            "host"
                | "authorization"
                | "proxy-authorization"
                | "upgrade"
                | "te"
                | "content-length"
                | "transfer-encoding"
                | "content-encoding"
        )
    }
}
