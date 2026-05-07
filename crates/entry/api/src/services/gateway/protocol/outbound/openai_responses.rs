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
pub struct OpenAiResponsesOutbound;

#[async_trait]
impl OutboundAdapter for OpenAiResponsesOutbound {
    fn provider_tag(&self) -> &'static str {
        "openai-responses"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = build_request_body(ctx.request, ctx.upstream_model);
        let url = format!("{}/responses", ctx.route.endpoint.trim_end_matches('/'));

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
            .map_err(|e| anyhow!("Upstream OpenAI Responses request failed: {e}"))?;
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
            .map_err(|e| anyhow!("Failed to read Responses body: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Responses body not valid JSON: {e}"))?;
        let canon = parse_response_object(&value, &ctx.request.model);
        Ok(OutboundOutcome::Buffered(canon))
    }
}

fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    let mut input: Vec<Value> = Vec::new();
    for msg in &request.messages {
        match msg.role {
            Role::Tool => {
                for part in &msg.content {
                    if let CanonicalContent::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = part
                    {
                        let output = flatten_text_parts(content);
                        input.push(json!({
                            "type": "function_call_output",
                            "call_id": tool_use_id,
                            "output": output,
                        }));
                    }
                }
            },
            Role::Assistant => {
                let mut text = String::new();
                let mut tool_calls: Vec<Value> = Vec::new();
                let mut reasoning: Option<String> = None;
                for part in &msg.content {
                    match part {
                        CanonicalContent::Text(t) => text.push_str(t),
                        CanonicalContent::ToolUse { id, name, input: arg } => {
                            tool_calls.push(json!({
                                "type": "function_call",
                                "call_id": id,
                                "name": name,
                                "arguments": serde_json::to_string(arg)
                                    .unwrap_or_else(|_| "{}".into()),
                            }));
                        },
                        CanonicalContent::Thinking { text: t, .. } => {
                            reasoning = Some(t.clone());
                        },
                        _ => {},
                    }
                }
                if let Some(r) = reasoning {
                    input.push(json!({
                        "type": "reasoning",
                        "summary": [{ "type": "summary_text", "text": r }],
                    }));
                }
                input.extend(tool_calls);
                if !text.is_empty() {
                    input.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": text }],
                    }));
                }
            },
            Role::User | Role::System => {
                let parts: Vec<Value> = msg
                    .content
                    .iter()
                    .filter_map(content_to_input_part)
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                input.push(json!({
                    "type": "message",
                    "role": match msg.role {
                        Role::System => "developer",
                        _ => "user",
                    },
                    "content": parts,
                }));
            },
        }
    }

    let mut obj = Map::new();
    obj.insert("model".into(), Value::String(upstream_model.to_string()));
    obj.insert("input".into(), Value::Array(input));
    obj.insert(
        "max_output_tokens".into(),
        Value::from(request.max_tokens),
    );
    if let Some(sys) = &request.system {
        obj.insert("instructions".into(), Value::String(sys.clone()));
    }
    if let Some(t) = request.temperature {
        obj.insert("temperature".into(), json!(t));
    }
    if let Some(p) = request.top_p {
        obj.insert("top_p".into(), json!(p));
    }
    if request.stream {
        obj.insert("stream".into(), Value::Bool(true));
    }
    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            })
            .collect();
        obj.insert("tools".into(), Value::Array(tools));
    }
    if let Some(tc) = &request.tool_choice {
        obj.insert("tool_choice".into(), tool_choice_to_responses(tc));
    }
    if let Some(thinking) = &request.thinking {
        if thinking.enabled {
            let effort = match thinking.budget_tokens {
                Some(b) if b >= 16384 => "high",
                Some(b) if b >= 4096 => "medium",
                Some(_) => "low",
                None => "medium",
            };
            obj.insert(
                "reasoning".into(),
                json!({ "effort": effort }),
            );
        }
    }
    Value::Object(obj)
}

fn content_to_input_part(part: &CanonicalContent) -> Option<Value> {
    match part {
        CanonicalContent::Text(t) => Some(json!({ "type": "input_text", "text": t })),
        CanonicalContent::Image(src) => match src {
            ImageSource::Url(u) => Some(json!({ "type": "input_image", "image_url": u })),
            ImageSource::Base64 { media_type, data } => Some(json!({
                "type": "input_image",
                "image_url": format!("data:{media_type};base64,{data}"),
            })),
        },
        _ => None,
    }
}

fn flatten_text_parts(parts: &[CanonicalContent]) -> String {
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

fn tool_choice_to_responses(tc: &CanonicalToolChoice) -> Value {
    match tc {
        CanonicalToolChoice::Auto => Value::String("auto".into()),
        CanonicalToolChoice::None => Value::String("none".into()),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => {
            Value::String("required".into())
        },
        CanonicalToolChoice::Tool(name) => json!({
            "type": "function",
            "name": name,
        }),
    }
}

fn parse_response_object(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("resp_{}", Uuid::new_v4().simple()));
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_string();
    let usage = value
        .get("usage")
        .map(|u| CanonicalUsage {
            input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
            match kind {
                "message" => {
                    if let Some(parts) = item.get("content").and_then(Value::as_array) {
                        for p in parts {
                            let ptype = p.get("type").and_then(Value::as_str).unwrap_or("");
                            if matches!(ptype, "output_text" | "text") {
                                if let Some(text) = p.get("text").and_then(Value::as_str) {
                                    content.push(CanonicalContent::Text(text.to_string()));
                                }
                            }
                        }
                    }
                },
                "function_call" => {
                    let id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("id").and_then(Value::as_str))
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let args = item.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                    let input: Value =
                        serde_json::from_str(args).unwrap_or(Value::Object(Map::new()));
                    content.push(CanonicalContent::ToolUse { id, name, input });
                },
                "reasoning" => {
                    let text = item
                        .get("summary")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.get("text").and_then(Value::as_str))
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    if !text.is_empty() {
                        content.push(CanonicalContent::Thinking {
                            text,
                            signature: None,
                        });
                    }
                },
                _ => {},
            }
        }
    }

    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_openai)
        .or(Some(CanonicalStopReason::EndTurn));

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
    let initial = ResponsesStreamState {
        buf: Vec::new(),
        model: fallback_model,
        started: false,
        items: Vec::new(),
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
                        let mut data_parts: Vec<&str> = Vec::new();
                        for line in frame_str.lines() {
                            if let Some(d) = line.strip_prefix("data: ") {
                                data_parts.push(d);
                            }
                        }
                        let joined = data_parts.join("\n");
                        if joined.trim().is_empty() {
                            continue;
                        }
                        if let Ok(value) = serde_json::from_str::<Value>(&joined) {
                            handle_responses_event(state, &value, &mut events);
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

struct ResponsesStreamState {
    buf: Vec<u8>,
    model: String,
    started: bool,
    items: Vec<ItemSlot>,
}

struct ItemSlot {
    output_index: i64,
    canonical_index: u32,
    kind: SlotKind,
}

enum SlotKind {
    Message,
    Function,
    Reasoning,
}

fn handle_responses_event(
    state: &mut ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "response.created" => {
            let response = value.get("response").unwrap_or(&Value::Null);
            let id = response
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("resp_unknown")
                .to_string();
            let model = response
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&state.model)
                .to_string();
            state.model = model.clone();
            state.started = true;
            events.push(Ok(CanonicalEvent::MessageStart {
                id,
                model,
                usage: CanonicalUsage::default(),
            }));
        },
        "response.output_item.added" => {
            let output_index = value.get("output_index").and_then(Value::as_i64).unwrap_or(-1);
            let item = value.get("item").unwrap_or(&Value::Null);
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            let canonical_index = state.items.len() as u32;
            match item_type {
                "message" => {
                    state.items.push(ItemSlot {
                        output_index,
                        canonical_index,
                        kind: SlotKind::Message,
                    });
                    events.push(Ok(CanonicalEvent::ContentBlockStart {
                        index: canonical_index,
                        block: ContentBlockKind::Text,
                    }));
                },
                "function_call" => {
                    let id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("id").and_then(Value::as_str))
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    state.items.push(ItemSlot {
                        output_index,
                        canonical_index,
                        kind: SlotKind::Function,
                    });
                    events.push(Ok(CanonicalEvent::ContentBlockStart {
                        index: canonical_index,
                        block: ContentBlockKind::ToolUse { id, name },
                    }));
                },
                "reasoning" => {
                    state.items.push(ItemSlot {
                        output_index,
                        canonical_index,
                        kind: SlotKind::Reasoning,
                    });
                    events.push(Ok(CanonicalEvent::ContentBlockStart {
                        index: canonical_index,
                        block: ContentBlockKind::Thinking { signature: None },
                    }));
                },
                _ => {},
            }
        },
        "response.output_text.delta" => {
            let output_index = value.get("output_index").and_then(Value::as_i64).unwrap_or(-1);
            let Some(idx) = lookup_canonical(state, output_index, SlotKindMatch::Message) else {
                return;
            };
            let delta = value.get("delta").and_then(Value::as_str).unwrap_or("");
            if !delta.is_empty() {
                events.push(Ok(CanonicalEvent::TextDelta {
                    index: idx,
                    text: delta.to_string(),
                }));
            }
        },
        "response.function_call_arguments.delta" => {
            let output_index = value.get("output_index").and_then(Value::as_i64).unwrap_or(-1);
            let Some(idx) = lookup_canonical(state, output_index, SlotKindMatch::Function) else {
                return;
            };
            let delta = value.get("delta").and_then(Value::as_str).unwrap_or("");
            if !delta.is_empty() {
                events.push(Ok(CanonicalEvent::ToolUseDelta {
                    index: idx,
                    partial_json: delta.to_string(),
                }));
            }
        },
        "response.reasoning_summary_text.delta" => {
            let output_index = value.get("output_index").and_then(Value::as_i64).unwrap_or(-1);
            let Some(idx) = lookup_canonical(state, output_index, SlotKindMatch::Reasoning) else {
                return;
            };
            let delta = value.get("delta").and_then(Value::as_str).unwrap_or("");
            if !delta.is_empty() {
                events.push(Ok(CanonicalEvent::ThinkingDelta {
                    index: idx,
                    text: delta.to_string(),
                }));
            }
        },
        "response.output_item.done" => {
            let output_index = value.get("output_index").and_then(Value::as_i64).unwrap_or(-1);
            if let Some(slot) = state
                .items
                .iter()
                .find(|s| s.output_index == output_index)
            {
                events.push(Ok(CanonicalEvent::ContentBlockStop {
                    index: slot.canonical_index,
                }));
            }
        },
        "response.completed" => {
            let response = value.get("response").unwrap_or(&Value::Null);
            if let Some(usage) = response.get("usage") {
                events.push(Ok(CanonicalEvent::UsageDelta(CanonicalUsage {
                    input_tokens: usage
                        .get("input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                    output_tokens: usage
                        .get("output_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                })));
            }
            events.push(Ok(CanonicalEvent::MessageStop {
                stop_reason: Some(CanonicalStopReason::EndTurn),
            }));
        },
        "response.failed" | "error" => {
            let msg = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream error")
                .to_string();
            events.push(Ok(CanonicalEvent::Error(msg)));
        },
        _ => {},
    }
    let _ = state.started;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotKindMatch {
    Message,
    Function,
    Reasoning,
}

fn lookup_canonical(
    state: &ResponsesStreamState,
    output_index: i64,
    want: SlotKindMatch,
) -> Option<u32> {
    state.items.iter().find_map(|s| {
        if s.output_index != output_index {
            return None;
        }
        let matches = matches!(
            (&s.kind, want),
            (SlotKind::Message, SlotKindMatch::Message)
                | (SlotKind::Function, SlotKindMatch::Function)
                | (SlotKind::Reasoning, SlotKindMatch::Reasoning)
        );
        if matches { Some(s.canonical_index) } else { None }
    })
}
