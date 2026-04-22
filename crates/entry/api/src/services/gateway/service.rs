use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use futures_util::StreamExt;
use http::header::CONTENT_TYPE;
use std::time::Instant;
use systemprompt_models::profile::{GatewayConfig, GatewayProvider, GatewayRoute};

use super::converter;
use super::models::{AnthropicGatewayRequest, AnthropicGatewayResponse};

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        request: AnthropicGatewayRequest,
        raw_body: Bytes,
    ) -> Result<Response<Body>> {
        let route = config
            .find_route(&request.model)
            .ok_or_else(|| anyhow!("No gateway route matches model '{}'", request.model))?;

        let secrets = systemprompt_models::SecretsBootstrap::get()
            .map_err(|e| anyhow!("Secrets not available: {e}"))?;

        let upstream_api_key = secrets.get(&route.api_key_secret).ok_or_else(|| {
            anyhow!(
                "Gateway API key secret '{}' not configured",
                route.api_key_secret
            )
        })?;

        let is_streaming = request.stream.unwrap_or(false);
        let start = Instant::now();

        tracing::info!(
            model = %request.model,
            provider = %route.provider.as_str(),
            upstream = %route.endpoint,
            streaming = is_streaming,
            "Gateway request dispatched"
        );

        let response = match route.provider {
            GatewayProvider::Anthropic => {
                proxy_anthropic(route, upstream_api_key, raw_body, is_streaming).await?
            },
            GatewayProvider::OpenAI | GatewayProvider::Moonshot | GatewayProvider::Qwen => {
                let upstream_model = route.effective_upstream_model(&request.model);
                let openai_req = converter::to_openai_request(&request, upstream_model);
                proxy_openai(
                    route,
                    upstream_api_key,
                    openai_req,
                    &request.model,
                    is_streaming,
                )
                .await?
            },
            GatewayProvider::Gemini => {
                return Err(anyhow!("Gemini upstream not yet implemented"));
            },
        };

        let latency = start.elapsed().as_millis();
        tracing::info!(
            latency_ms = latency,
            model = %request.model,
            "Gateway request completed"
        );

        Ok(response)
    }
}

async fn proxy_anthropic(
    route: &GatewayRoute,
    api_key: &str,
    body: Bytes,
    streaming: bool,
) -> Result<Response<Body>> {
    let client = reqwest::Client::new();
    let url = format!("{}/messages", route.endpoint.trim_end_matches('/'));

    let mut req = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(body);

    for (name, value) in &route.extra_headers {
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

    if streaming {
        let stream = upstream_response.bytes_stream().map(|chunk| {
            chunk.map_err(|e| {
                tracing::warn!(error = %e, "Anthropic stream chunk error");
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)
            })
        });

        let body = Body::from_stream(stream);
        return Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("cache-control", "no-cache")
            .header("x-accel-buffering", "no")
            .body(body)
            .map_err(|e| anyhow!("Response build error: {e}"));
    }

    let response_bytes = upstream_response
        .bytes()
        .await
        .map_err(|e| anyhow!("Failed to read Anthropic response: {e}"))?;

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, upstream_content_type)
        .body(Body::from(response_bytes))
        .map_err(|e| anyhow!("Response build error: {e}"))
}

async fn proxy_openai(
    route: &GatewayRoute,
    api_key: &str,
    openai_request: super::models::OpenAiGatewayRequest,
    model: &str,
    streaming: bool,
) -> Result<Response<Body>> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", route.endpoint.trim_end_matches('/'));

    let mut req = client
        .post(&url)
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&openai_request);

    for (name, value) in &route.extra_headers {
        req = req.header(name.as_str(), value.as_str());
    }

    let upstream_response = req
        .send()
        .await
        .map_err(|e| anyhow!("Upstream OpenAI-compatible request failed: {e}"))?;

    let status = upstream_response.status();

    if streaming {
        if !status.is_success() {
            let err = upstream_response.text().await.unwrap_or_default();
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        let model_str = model.to_string();
        let stream = upstream_response
            .bytes_stream()
            .map(move |chunk| match chunk {
                Ok(bytes) => Ok(openai_sse_to_anthropic_sse(&bytes, &model_str)),
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)),
            });

        let body = Body::from_stream(stream);
        return Response::builder()
            .status(200)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("cache-control", "no-cache")
            .header("x-accel-buffering", "no")
            .body(body)
            .map_err(|e| anyhow!("Response build error: {e}"));
    }

    if !status.is_success() {
        let err = upstream_response.text().await.unwrap_or_default();
        return Err(anyhow!("Upstream error {status}: {err}"));
    }

    let openai_resp: super::models::OpenAiGatewayResponse = upstream_response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to deserialize upstream response: {e}"))?;

    let anthropic_resp: AnthropicGatewayResponse =
        converter::from_openai_response(openai_resp, model);

    let body_bytes =
        serde_json::to_vec(&anthropic_resp).map_err(|e| anyhow!("Serialization error: {e}"))?;

    Response::builder()
        .status(200)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body_bytes))
        .map_err(|e| anyhow!("Response build error: {e}"))
}

fn openai_sse_to_anthropic_sse(bytes: &Bytes, model: &str) -> Bytes {
    let text = String::from_utf8_lossy(bytes);
    let mut output = String::new();

    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data.trim() == "[DONE]" {
            push_sse_frame(&mut output, &serde_json::json!({ "type": "message_stop" }));
            continue;
        }
        let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) else {
            continue;
        };
        append_anthropic_frames(&mut output, &chunk, model);
    }

    Bytes::from(output)
}

fn append_anthropic_frames(output: &mut String, chunk: &OpenAiStreamChunk, model: &str) {
    for choice in &chunk.choices {
        if choice.delta.role.is_some() {
            let id = chunk.id.as_deref().unwrap_or("msg_openai");
            push_sse_frame(
                output,
                &serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": id,
                        "type": "message",
                        "role": "assistant",
                        "model": model,
                        "usage": { "input_tokens": 0, "output_tokens": 0 },
                    },
                }),
            );
            push_sse_frame(
                output,
                &serde_json::json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "text", "text": "" },
                }),
            );
        }
        if let Some(text) = choice.delta.content.as_deref() {
            if !text.is_empty() {
                push_sse_frame(
                    output,
                    &serde_json::json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": { "type": "text_delta", "text": text },
                    }),
                );
            }
        }
        if let Some(finish) = choice.finish_reason.as_deref() {
            if !finish.is_empty() && finish != "null" {
                let stop_reason = if finish == "stop" { "end_turn" } else { finish };
                push_sse_frame(
                    output,
                    &serde_json::json!({
                        "type": "message_delta",
                        "delta": { "stop_reason": stop_reason },
                        "usage": { "output_tokens": 0 },
                    }),
                );
            }
        }
    }
}

fn push_sse_frame(output: &mut String, value: &serde_json::Value) {
    output.push_str("data: ");
    if let Ok(encoded) = serde_json::to_string(value) {
        output.push_str(&encoded);
    }
    output.push_str("\n\n");
}

#[derive(serde::Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(serde::Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
}
