use anyhow::{Result, anyhow};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::super::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource, Role,
};
use super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use super::{OutboundAdapter, OutboundCtx, OutboundOutcome};

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiChatOutbound;

#[async_trait]
impl OutboundAdapter for OpenAiChatOutbound {
    fn provider_tag(&self) -> &'static str {
        "openai"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = build_request_body(ctx.request, ctx.upstream_model);
        let url = format!(
            "{}/chat/completions",
            ctx.route.endpoint.trim_end_matches('/')
        );

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("authorization", format!("Bearer {}", ctx.api_key))
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream OpenAI-compatible request failed: {e}"))?;

        let status = upstream_response.status();
        if !status.is_success() {
            let err = upstream_response.text().await.unwrap_or_default();
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream = sse_to_canonical_events(stream, ctx.request.model.clone());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read OpenAI response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))?;
        let canon = parse_response(&value, &ctx.request.model);
        Ok(OutboundOutcome::Buffered(canon))
    }
}

fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(sys) = &request.system {
        messages.push(json!({ "role": "system", "content": sys }));
    }
    for msg in &request.messages {
        messages.extend(canonical_message_to_chat(msg));
    }

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_string()));
    obj.insert("messages".into(), Value::Array(messages));
    obj.insert("max_tokens".into(), Value::from(request.max_tokens));
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if !request.stop_sequences.is_empty() {
        obj.insert("stop".into(), json!(request.stop_sequences));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
        obj.insert(
            "stream_options".into(),
            json!({ "include_usage": true }),
        );
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    },
                })
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_chat(tc));
    }
    Value::Object(obj)
}

fn canonical_message_to_chat(msg: &CanonicalMessage) -> Vec<Value> {
    match msg.role {
        Role::System => vec![json!({
            "role": "system",
            "content": flatten_text(&msg.content),
        })],
        Role::User => {
            let parts: Vec<Value> = msg
                .content
                .iter()
                .filter_map(content_to_chat_part)
                .collect();
            if parts.iter().all(is_text_part) {
                let text = parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("");
                vec![json!({ "role": "user", "content": text })]
            } else {
                vec![json!({ "role": "user", "content": parts })]
            }
        },
        Role::Assistant => {
            let mut text = String::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            for part in &msg.content {
                match part {
                    CanonicalContent::Text(t) => text.push_str(t),
                    CanonicalContent::ToolUse { id, name, input } => {
                        tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(input)
                                    .unwrap_or_else(|_| "{}".into()),
                            },
                        }));
                    },
                    _ => {},
                }
            }
            let mut obj = Map::new();
            obj.insert("role".into(), Value::String("assistant".into()));
            if !text.is_empty() {
                obj.insert("content".into(), Value::String(text));
            } else {
                obj.insert("content".into(), Value::Null);
            }
            if !tool_calls.is_empty() {
                obj.insert("tool_calls".into(), Value::Array(tool_calls));
            }
            vec![Value::Object(obj)]
        },
        Role::Tool => msg
            .content
            .iter()
            .filter_map(|c| match c {
                CanonicalContent::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => Some(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": flatten_text(content),
                })),
                _ => None,
            })
            .collect(),
    }
}

fn content_to_chat_part(part: &CanonicalContent) -> Option<Value> {
    match part {
        CanonicalContent::Text(t) => Some(json!({ "type": "text", "text": t })),
        CanonicalContent::Image(src) => Some(json!({
            "type": "image_url",
            "image_url": {
                "url": match src {
                    ImageSource::Url(u) => u.clone(),
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    },
                },
            },
        })),
        _ => None,
    }
}

fn is_text_part(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("text")
}

fn flatten_text(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for p in parts {
        if let CanonicalContent::Text(t) = p {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

fn tool_choice_to_chat(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => Value::String("auto".into()),
        CanonicalToolChoice::None => Value::String("none".into()),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => {
            Value::String("required".into())
        },
        CanonicalToolChoice::Tool(name) => json!({
            "type": "function",
            "function": { "name": name },
        }),
    }
}

fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_string();

    let usage = value
        .get("usage")
        .map(|u| CanonicalUsage {
            input_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
        })
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    let mut stop_reason = None;

    if let Some(choice) = value.get("choices").and_then(Value::as_array).and_then(|a| a.first()) {
        stop_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(CanonicalStopReason::from_openai);
        if let Some(msg) = choice.get("message") {
            if let Some(text) = msg.get("content").and_then(Value::as_str) {
                if !text.is_empty() {
                    content.push(CanonicalContent::Text(text.to_string()));
                }
            }
            if let Some(tool_calls) = msg.get("tool_calls").and_then(Value::as_array) {
                for tc in tool_calls {
                    let id = tc
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let func = tc.get("function").unwrap_or(&Value::Null);
                    let name = func
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let args = func.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                    let input: Value =
                        serde_json::from_str(args).unwrap_or(Value::Object(Map::new()));
                    content.push(CanonicalContent::ToolUse { id, name, input });
                }
            }
        }
    }

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    }
}

fn sse_to_canonical_events<S>(
    stream: S,
    fallback_model: String,
) -> futures_util::stream::BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures_util::stream;
    let initial = OpenAiChatStreamState {
        buf: Vec::new(),
        model: fallback_model,
        started: false,
        text_block_open: false,
        next_index: 0,
        tool_calls: Vec::new(),
    };

    let s = stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan(initial, |state, item| {
            let res = match item {
                Ok(bytes) => {
                    state.buf.extend_from_slice(&bytes);
                    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
                    while let Some(pos) = find_double_newline(&state.buf) {
                        let frame: Vec<u8> = state.buf.drain(..pos + 2).collect();
                        let frame_str = String::from_utf8_lossy(&frame);
                        for line in frame_str.lines() {
                            let Some(data) = line.strip_prefix("data: ") else {
                                continue;
                            };
                            if data.trim() == "[DONE]" {
                                if state.text_block_open {
                                    events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
                                    state.text_block_open = false;
                                }
                                events.push(Ok(CanonicalEvent::MessageStop {
                                    stop_reason: Some(CanonicalStopReason::EndTurn),
                                }));
                                continue;
                            }
                            let Ok(value) = serde_json::from_str::<Value>(data) else {
                                continue;
                            };
                            handle_chunk(state, &value, &mut events);
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

fn handle_chunk(
    state: &mut OpenAiChatStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if !state.started {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("msg_openai")
            .to_string();
        let model = value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&state.model)
            .to_string();
        events.push(Ok(CanonicalEvent::MessageStart {
            id,
            model: model.clone(),
            usage: CanonicalUsage::default(),
        }));
        state.model = model;
        state.started = true;
    }

    if let Some(usage) = value.get("usage") {
        let u = CanonicalUsage {
            input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
        };
        events.push(Ok(CanonicalEvent::UsageDelta(u)));
    }

    let Some(choice) = value.get("choices").and_then(Value::as_array).and_then(|a| a.first()) else {
        return;
    };
    let delta = choice.get("delta").unwrap_or(&Value::Null);

    if let Some(text) = delta.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            if !state.text_block_open {
                events.push(Ok(CanonicalEvent::ContentBlockStart {
                    index: 0,
                    block: ContentBlockKind::Text,
                }));
                state.text_block_open = true;
                if state.next_index == 0 {
                    state.next_index = 1;
                }
            }
            events.push(Ok(CanonicalEvent::TextDelta {
                index: 0,
                text: text.to_string(),
            }));
        }
    }

    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            let provider_index = tc.get("index").and_then(Value::as_i64).unwrap_or(-1);
            let existing = state
                .tool_calls
                .iter()
                .find(|p| p.provider_index == provider_index)
                .map(|p| p.index);
            let canonical_index = if let Some(idx) = existing {
                idx
            } else {
                let idx = state.next_index;
                state.next_index += 1;
                let id = tc.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                events.push(Ok(CanonicalEvent::ContentBlockStart {
                    index: idx,
                    block: ContentBlockKind::ToolUse { id, name },
                }));
                state.tool_calls.push(ToolCallProgress {
                    index: idx,
                    provider_index,
                });
                idx
            };
            if let Some(args) = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
            {
                if !args.is_empty() {
                    events.push(Ok(CanonicalEvent::ToolUseDelta {
                        index: canonical_index,
                        partial_json: args.to_string(),
                    }));
                }
            }
        }
    }

    if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
        if state.text_block_open {
            events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
            state.text_block_open = false;
        }
        for tc in state.tool_calls.drain(..) {
            events.push(Ok(CanonicalEvent::ContentBlockStop { index: tc.index }));
        }
        events.push(Ok(CanonicalEvent::MessageStop {
            stop_reason: Some(CanonicalStopReason::from_openai(finish)),
        }));
    }
}

struct OpenAiChatStreamState {
    buf: Vec<u8>,
    model: String,
    started: bool,
    text_block_open: bool,
    next_index: u32,
    tool_calls: Vec<ToolCallProgress>,
}

struct ToolCallProgress {
    index: u32,
    provider_index: i64,
}
