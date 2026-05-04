mod sse;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use futures_util::StreamExt;
use http::header::CONTENT_TYPE;
use systemprompt_models::profile::GatewayRoute;

use super::converter;
use super::models::{AnthropicGatewayRequest, AnthropicGatewayResponse};

#[derive(Debug)]
pub struct UpstreamCtx<'a> {
    pub route: &'a GatewayRoute,
    pub api_key: &'a str,
    pub raw_body: Bytes,
    pub request: &'a AnthropicGatewayRequest,
    pub is_streaming: bool,
}

#[allow(missing_debug_implementations)]
pub enum UpstreamOutcome {
    Buffered {
        status: http::StatusCode,
        content_type: String,
        body: Bytes,
        served_model: Option<String>,
    },
    Streaming {
        status: http::StatusCode,
        stream: futures_util::stream::BoxStream<'static, Result<Bytes, std::io::Error>>,
    },
}

impl UpstreamOutcome {
    pub const fn status(&self) -> http::StatusCode {
        match self {
            Self::Buffered { status, .. } | Self::Streaming { status, .. } => *status,
        }
    }
}

#[async_trait]
pub trait GatewayUpstream: Send + Sync {
    async fn proxy(&self, ctx: UpstreamCtx<'_>) -> Result<UpstreamOutcome>;
}

#[derive(Debug, Clone, Copy)]
pub struct AnthropicCompatibleUpstream;

#[async_trait]
impl GatewayUpstream for AnthropicCompatibleUpstream {
    async fn proxy(&self, ctx: UpstreamCtx<'_>) -> Result<UpstreamOutcome> {
        let client = reqwest::Client::new();
        let url = format!("{}/messages", ctx.route.endpoint.trim_end_matches('/'));

        let upstream_model = ctx.route.effective_upstream_model(&ctx.request.model);
        let body_to_send: Bytes = if upstream_model == ctx.request.model {
            ctx.raw_body
        } else {
            super::flatten::rewrite_request_model(ctx.raw_body, upstream_model)?
        };

        let mut req = client
            .post(&url)
            .header("x-api-key", ctx.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(body_to_send);

        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream Anthropic request failed: {e}"))?;

        let status = upstream_response.status();
        let upstream_content_type = upstream_response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        if ctx.is_streaming {
            let stream = upstream_response.bytes_stream().map(|chunk| {
                chunk.map_err(|e| {
                    tracing::warn!(error = %e, "Anthropic stream chunk error");
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)
                })
            });
            return Ok(UpstreamOutcome::Streaming {
                status,
                stream: stream.boxed(),
            });
        }

        let response_bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Anthropic response: {e}"))?;

        let served_model = super::flatten::parse_served_model(&response_bytes);

        Ok(UpstreamOutcome::Buffered {
            status,
            content_type: upstream_content_type,
            body: response_bytes,
            served_model,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatibleUpstream;

#[async_trait]
impl GatewayUpstream for OpenAiCompatibleUpstream {
    async fn proxy(&self, ctx: UpstreamCtx<'_>) -> Result<UpstreamOutcome> {
        let upstream_model = ctx.route.effective_upstream_model(&ctx.request.model);
        let openai_request = converter::to_openai_request(ctx.request, upstream_model);
        let model = ctx.request.model.as_str();

        let client = reqwest::Client::new();
        let url = format!(
            "{}/chat/completions",
            ctx.route.endpoint.trim_end_matches('/')
        );

        let mut req = client
            .post(&url)
            .header("authorization", format!("Bearer {}", ctx.api_key))
            .header("content-type", "application/json")
            .json(&openai_request);

        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream OpenAI-compatible request failed: {e}"))?;

        let status = upstream_response.status();

        if ctx.is_streaming {
            if !status.is_success() {
                let err = upstream_response.text().await.unwrap_or_default();
                return Err(anyhow!("Upstream error {status}: {err}"));
            }

            let model_str = model.to_string();
            let stream = upstream_response
                .bytes_stream()
                .map(move |chunk| match chunk {
                    Ok(bytes) => Ok(sse::openai_sse_to_anthropic_sse(&bytes, &model_str)),
                    Err(e) => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)),
                });
            return Ok(UpstreamOutcome::Streaming {
                status: http::StatusCode::OK,
                stream: stream.boxed(),
            });
        }

        if !status.is_success() {
            let err = upstream_response.text().await.unwrap_or_default();
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        let openai_resp: super::models::OpenAiGatewayResponse = upstream_response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to deserialize upstream response: {e}"))?;

        let served_model = Some(openai_resp.model.clone());

        let anthropic_resp: AnthropicGatewayResponse =
            converter::from_openai_response(openai_resp, model);

        let body_bytes =
            serde_json::to_vec(&anthropic_resp).map_err(|e| anyhow!("Serialization error: {e}"))?;

        Ok(UpstreamOutcome::Buffered {
            status: http::StatusCode::OK,
            content_type: "application/json".to_string(),
            body: Bytes::from(body_bytes),
            served_model,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GatewayUpstreamRegistration {
    pub tag: &'static str,
    pub factory: fn() -> Arc<dyn GatewayUpstream>,
}

inventory::collect!(GatewayUpstreamRegistration);

pub fn build_response(outcome: UpstreamOutcome) -> Response<Body> {
    match outcome {
        UpstreamOutcome::Buffered {
            status,
            content_type,
            body,
            served_model: _,
        } => Response::builder()
            .status(status)
            .header(CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap_or_else(|_| Response::new(Body::empty())),
        UpstreamOutcome::Streaming { status, stream } => Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("cache-control", "no-cache")
            .header("x-accel-buffering", "no")
            .body(Body::from_stream(stream))
            .unwrap_or_else(|_| Response::new(Body::empty())),
    }
}
