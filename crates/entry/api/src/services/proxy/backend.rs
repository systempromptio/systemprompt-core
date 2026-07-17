//! HTTP plumbing for forwarding a proxied request to a local backend.
//!
//! Splits the forwarding pipeline into focused helpers: [`HeaderInjector`]
//! propagates request-context headers, [`UrlResolver`] builds the backend URL,
//! [`RequestBuilder`] constructs the outbound `reqwest` request (filtering
//! hop-by-hop and invalid auth headers), and [`ResponseHandler`] streams the
//! response back, wrapping SSE bodies in a keep-alive stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use futures_util::TryStreamExt;
use reqwest::Method;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{Context, Poll};
use std::time::Duration;
use systemprompt_models::RequestContext;
use systemprompt_traits::InjectContextHeaders;
use tokio::time::{Instant, Sleep};

pub(super) use super::errors::ProxyError;

#[derive(Debug, Clone, Copy)]
pub(super) struct HeaderInjector;

impl HeaderInjector {
    pub(super) fn inject_context(headers: &mut HeaderMap, req_ctx: &RequestContext) {
        req_ctx.inject_headers(headers);
    }
}

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

#[derive(Debug, Clone, Copy)]
pub(super) struct RequestBuilder;

impl RequestBuilder {
    pub(super) async fn extract_body(body: Body) -> Result<Vec<u8>, axum::Error> {
        const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

        to_bytes(body, MAX_BODY_SIZE)
            .await
            .map(|bytes| bytes.to_vec())
    }

    pub(super) fn parse_method(method_str: &str) -> Result<Method, String> {
        Method::from_str(method_str)
            .map_err(|e| format!("Invalid HTTP method '{}': {}", method_str, e))
    }

    pub(super) fn build_request(
        client: &reqwest::Client,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Vec<u8>,
    ) -> reqwest::RequestBuilder {
        let mut req_builder = client.request(method, url);
        req_builder = Self::add_headers(req_builder, headers);

        if !body.is_empty() {
            req_builder = req_builder.body(body);
        }

        req_builder
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
        matches!(lower_name.as_str(), "host" | "x-mcp-proxy-auth")
    }

    fn is_valid_auth_header(value: &str) -> bool {
        value != "Bearer" && !value.trim().eq_ignore_ascii_case("bearer")
    }
}

pub(super) const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_KEEPALIVE_PAYLOAD: &[u8] = b": keepalive\n\n";

pub(super) struct SseKeepaliveStream<S> {
    inner: S,
    keepalive_interval: Duration,
    deadline: Pin<Box<Sleep>>,
}

impl<S> SseKeepaliveStream<S> {
    pub(super) fn new(inner: S, keepalive_interval: Duration) -> Self {
        Self {
            inner,
            keepalive_interval,
            deadline: Box::pin(tokio::time::sleep(keepalive_interval)),
        }
    }
}

impl<S> futures_util::Stream for SseKeepaliveStream<S>
where
    S: futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let interval = self.keepalive_interval;
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(item)) => {
                self.deadline.as_mut().reset(Instant::now() + interval);
                Poll::Ready(Some(item))
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => match self.deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    self.deadline.as_mut().reset(Instant::now() + interval);
                    Poll::Ready(Some(Ok(Bytes::from_static(SSE_KEEPALIVE_PAYLOAD))))
                },
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResponseHandler;

impl ResponseHandler {
    pub(super) fn build_response(response: reqwest::Response) -> Result<Response<Body>, String> {
        let status_code = response.status().as_u16();
        let axum_status = StatusCode::from_u16(status_code)
            .map_err(|e| format!("Invalid status code {}: {}", status_code, e))?;

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
    ) -> Result<Response<Body>, String> {
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
            .map_err(|e| format!("Failed to build response body: {}", e))
    }

    fn should_preserve_header(key: &str) -> bool {
        match key.to_lowercase().as_str() {
            "host" | "authorization" | "proxy-authorization" | "upgrade" | "te" => false,
            header if header.starts_with("x-mcp-") => true,
            _ => true,
        }
    }
}
