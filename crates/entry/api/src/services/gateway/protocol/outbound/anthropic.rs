use anyhow::{Result, anyhow};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{Map, Value, json};

use super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource, Role,
};
use super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use super::super::inbound::anthropic_messages::content_to_anthropic_block;
use super::{OutboundAdapter, OutboundCtx, OutboundOutcome};

#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicOutbound;

#[async_trait]
impl OutboundAdapter for AnthropicOutbound {
    fn provider_tag(&self) -> &'static str {
        "anthropic"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = build_request_body(ctx.request, ctx.upstream_model);
        let url = format!("{}/messages", ctx.route.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("x-api-key", ctx.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }
        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream Anthropic request failed: {e}"))?;

        let status = upstream_response.status();

        if ctx.request.stream {
            if !status.is_success() {
                let err = upstream_response.text().await.unwrap_or_default();
                return Err(anyhow!("Upstream error {status}: {err}"));
            }
            let stream = upstream_response.bytes_stream();
            let event_stream = sse_to_canonical_events(stream);
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        if !status.is_success() {
            let err = upstream_response.text().await.unwrap_or_default();
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Anthropic response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Anthropic response not valid JSON: {e}"))?;
        let canon = parse_response(&value, ctx.request.model.as_str())?;
        Ok(OutboundOutcome::Buffered(canon))
    }
}

fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .filter(|m| !matches!(m.role, Role::System))
        .map(canonical_message_to_anthropic)
        .collect();

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_string()));
    obj.insert("max_tokens".into(), Value::from(request.max_tokens));
    obj.insert("messages".into(), Value::Array(messages));
    if let Some(sys) = &request.system {
        obj.insert("system".into(), Value::String(sys.clone()));
    }
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if let Some(k) = request.top_k {
        obj.insert("top_k".into(), json!(k));
    }
    if !request.stop_sequences.is_empty() {
        obj.insert("stop_sequences".into(), json!(request.stop_sequences));
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                let mut tobj = Map::new();
                tobj.insert("name".into(), Value::String(t.name.clone()));
                if let Some(d) = &t.description {
                    tobj.insert("description".into(), Value::String(d.clone()));
                }
                tobj.insert("input_schema".into(), t.input_schema.clone());
                Value::Object(tobj)
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_anthropic(tc));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
    }
    if let Some(thinking) = &request.thinking {
        if thinking.enabled {
            let mut t = Map::new();
            t.insert("type".into(), Value::String("enabled".into()));
            if let Some(b) = thinking.budget_tokens {
                t.insert("budget_tokens".into(), Value::from(b));
            }
            obj.insert("thinking".into(), Value::Object(t));
        }
    }
    if let Some(meta) = &request.metadata {
        obj.insert("metadata".into(), meta.clone());
    }
    Value::Object(obj)
}

fn canonical_message_to_anthropic(msg: &CanonicalMessage) -> Value {
    let role = match msg.role {
        Role::User | Role::Tool => "user",
        Role::Assistant => "assistant",
        Role::System => "user",
    };
    let content: Vec<Value> = msg
        .content
        .iter()
        .filter_map(content_to_anthropic_block)
        .collect();
    json!({ "role": role, "content": content })
}

fn tool_choice_to_anthropic(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => json!({ "type": "auto" }),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => json!({ "type": "any" }),
        CanonicalToolChoice::None => json!({ "type": "none" }),
        CanonicalToolChoice::Tool(name) => json!({ "type": "tool", "name": name }),
    }
}

fn parse_response(value: &Value, fallback_model: &str) -> Result<CanonicalResponse> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_string();
    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_anthropic);
    let usage = value
        .get("usage")
        .map(|u| CanonicalUsage {
            input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    if let Some(arr) = value.get("content").and_then(Value::as_array) {
        for block in arr {
            if let Some(part) = parse_content_block(block) {
                content.push(part);
            }
        }
    }

    Ok(CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    })
}

pub(super) fn parse_content_block(value: &Value) -> Option<CanonicalContent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "text" => Some(CanonicalContent::Text(
            value.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
        )),
        "thinking" => Some(CanonicalContent::Thinking {
            text: value
                .get("thinking")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "tool_use" => Some(CanonicalContent::ToolUse {
            id: value.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: value.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "image" => {
            let src = value.get("source")?;
            let stype = src.get("type").and_then(Value::as_str)?;
            match stype {
                "base64" => Some(CanonicalContent::Image(ImageSource::Base64 {
                    media_type: src
                        .get("media_type")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png")
                        .to_string(),
                    data: src
                        .get("data")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                })),
                "url" => Some(CanonicalContent::Image(ImageSource::Url(
                    src.get("url").and_then(Value::as_str).unwrap_or("").to_string(),
                ))),
                _ => None,
            }
        },
        _ => None,
    }
}

fn sse_to_canonical_events<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures_util::stream;
    let s = stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan(Vec::<u8>::new(), |buf, item| {
            let res = match item {
                Ok(bytes) => {
                    buf.extend_from_slice(&bytes);
                    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
                    while let Some(pos) = find_double_newline(buf) {
                        let frame: Vec<u8> = buf.drain(..pos + 2).collect();
                        let frame_str = String::from_utf8_lossy(&frame);
                        for line in frame_str.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data.trim() == "[DONE]" {
                                    continue;
                                }
                                if let Ok(value) = serde_json::from_str::<Value>(data) {
                                    if let Some(ev) = anthropic_event_to_canonical(&value) {
                                        events.push(Ok(ev));
                                    }
                                }
                            }
                        }
                    }
                    Some(events)
                },
                Err(e) => Some(vec![Err(e)]),
            };
            futures_util::future::ready(res)
        })
        .flat_map(stream::iter);
    s.boxed()
}

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn anthropic_event_to_canonical(value: &Value) -> Option<CanonicalEvent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "message_start" => {
            let msg = value.get("message")?;
            Some(CanonicalEvent::MessageStart {
                id: msg
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                model: msg
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                usage: usage_from_value(msg.get("usage")),
            })
        },
        "content_block_start" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            let block = value.get("content_block")?;
            let block_type = block.get("type").and_then(Value::as_str)?;
            let kind = match block_type {
                "text" => ContentBlockKind::Text,
                "thinking" => ContentBlockKind::Thinking {
                    signature: block
                        .get("signature")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                },
                "tool_use" => ContentBlockKind::ToolUse {
                    id: block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                },
                _ => return None,
            };
            Some(CanonicalEvent::ContentBlockStart { index, block: kind })
        },
        "content_block_delta" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            let delta = value.get("delta")?;
            let dtype = delta.get("type").and_then(Value::as_str)?;
            match dtype {
                "text_delta" => Some(CanonicalEvent::TextDelta {
                    index,
                    text: delta
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                "thinking_delta" => Some(CanonicalEvent::ThinkingDelta {
                    index,
                    text: delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                "input_json_delta" => Some(CanonicalEvent::ToolUseDelta {
                    index,
                    partial_json: delta
                        .get("partial_json")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                _ => None,
            }
        },
        "content_block_stop" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            Some(CanonicalEvent::ContentBlockStop { index })
        },
        "message_delta" => {
            let stop_reason = value
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(Value::as_str)
                .map(CanonicalStopReason::from_anthropic);
            let usage = value.get("usage").map(|u| usage_from_value(Some(u)));
            if stop_reason.is_some() {
                return Some(CanonicalEvent::MessageStop { stop_reason });
            }
            usage.map(CanonicalEvent::UsageDelta)
        },
        "message_stop" => Some(CanonicalEvent::MessageStop { stop_reason: None }),
        "error" => {
            let msg = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream error")
                .to_string();
            Some(CanonicalEvent::Error(msg))
        },
        _ => None,
    }
}

fn usage_from_value(v: Option<&Value>) -> CanonicalUsage {
    let Some(u) = v else {
        return CanonicalUsage::default();
    };
    CanonicalUsage {
        input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
    }
}
