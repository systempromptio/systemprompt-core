use futures_util::StreamExt;
// JSON: protocol boundary — Anthropic Messages outbound wire format is dynamic
// JSON.
use serde_json::Value;

use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

#[cfg_attr(not(feature = "test-api"), expect(unreachable_pub, reason = "items are re-exported via `test_api` only when the feature is on"))]
pub fn sse_to_canonical_events<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures_util::stream;
    let s = stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan((Vec::<u8>::new(), String::new()), |state, item| {
            let (buf, msg_id) = state;
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
                                    if let Some(ev) = anthropic_event_to_canonical(&value, msg_id) {
                                        if let CanonicalEvent::MessageStart { id, .. } = &ev {
                                            msg_id.clone_from(id);
                                        }
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

fn anthropic_event_to_canonical(value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "message_start" => convert_message_start(value),
        "content_block_start" => convert_content_block_start(value),
        "content_block_delta" => convert_content_block_delta(value),
        "content_block_stop" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            Some(CanonicalEvent::ContentBlockStop { index })
        },
        "message_delta" => convert_message_delta(value, msg_id),
        "message_stop" => Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason: None,
        }),
        "error" => {
            let msg = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream error")
                .to_owned();
            Some(CanonicalEvent::Error(msg))
        },
        _ => None,
    }
}

fn convert_message_start(value: &Value) -> Option<CanonicalEvent> {
    let msg = value.get("message")?;
    Some(CanonicalEvent::MessageStart {
        id: msg
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        model: msg
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        usage: usage_from_value(msg.get("usage")),
    })
}

fn convert_content_block_start(value: &Value) -> Option<CanonicalEvent> {
    let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
    let block = value.get("content_block")?;
    let block_type = block.get("type").and_then(Value::as_str)?;
    let kind = match block_type {
        "text" => ContentBlockKind::Text,
        "thinking" => ContentBlockKind::Thinking {
            signature: block
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
        "tool_use" => ContentBlockKind::ToolUse {
            id: block
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            name: block
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        },
        _ => return None,
    };
    Some(CanonicalEvent::ContentBlockStart { index, block: kind })
}

fn convert_content_block_delta(value: &Value) -> Option<CanonicalEvent> {
    let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
    let delta = value.get("delta")?;
    let dtype = delta.get("type").and_then(Value::as_str)?;
    let text_field = |field: &str| {
        delta
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    match dtype {
        "text_delta" => Some(CanonicalEvent::TextDelta {
            index,
            text: text_field("text"),
        }),
        "thinking_delta" => Some(CanonicalEvent::ThinkingDelta {
            index,
            text: text_field("thinking"),
        }),
        "input_json_delta" => Some(CanonicalEvent::ToolUseDelta {
            index,
            partial_json: text_field("partial_json"),
        }),
        _ => None,
    }
}

fn convert_message_delta(value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    let stop_reason = value
        .get("delta")
        .and_then(|d| d.get("stop_reason"))
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_anthropic);
    let usage = value.get("usage").map(|u| usage_from_value(Some(u)));
    if stop_reason.is_some() {
        return Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason,
        });
    }
    usage.map(CanonicalEvent::UsageDelta)
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
