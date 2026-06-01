//! Anthropic Messages response + SSE-frame parse side of the codec.
//!
//! [`parse_response`] deserializes a buffered Messages reply into a
//! [`CanonicalResponse`]; [`event_from_sse`] turns one decoded SSE `data:`
//! payload into a [`CanonicalEvent`]. The streaming side stays dynamic because
//! each frame is a distinct, sparsely-populated event keyed on `type`.

use serde::Deserialize;
use serde_json::Value;

use crate::wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
    ContentBlockKind, ImageSource,
};

#[derive(Debug, Default, Deserialize)]
struct AnthropicResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
    #[serde(default)]
    content: Vec<AnthropicBlock>,
}

#[derive(Debug, Default, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },
    ToolUse {
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        input: Value,
    },
    Image {
        source: AnthropicImageSource,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicImageSource {
    Base64 {
        #[serde(default)]
        media_type: Option<String>,
        #[serde(default)]
        data: String,
    },
    Url {
        #[serde(default)]
        url: String,
    },
    #[serde(other)]
    Unknown,
}

#[must_use]
pub fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let resp = AnthropicResponse::deserialize(value).unwrap_or_default();
    let id = resp.id.unwrap_or_default();
    let model = resp.model.unwrap_or_else(|| fallback_model.to_owned());
    let stop_reason = resp
        .stop_reason
        .as_deref()
        .map(CanonicalStopReason::from_anthropic);
    let usage = resp
        .usage
        .map_or_else(CanonicalUsage::default, |u| CanonicalUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });

    let content = resp
        .content
        .into_iter()
        .filter_map(canonical_block)
        .collect();

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    }
}

fn canonical_block(block: AnthropicBlock) -> Option<CanonicalContent> {
    match block {
        AnthropicBlock::Text { text } => Some(CanonicalContent::Text(text)),
        AnthropicBlock::Thinking {
            thinking,
            signature,
        } => Some(CanonicalContent::Thinking {
            text: thinking,
            signature,
        }),
        AnthropicBlock::ToolUse { id, name, input } => {
            Some(CanonicalContent::ToolUse { id, name, input })
        },
        AnthropicBlock::Image { source } => canonical_image(source),
        AnthropicBlock::Unknown => None,
    }
}

fn canonical_image(source: AnthropicImageSource) -> Option<CanonicalContent> {
    match source {
        AnthropicImageSource::Base64 { media_type, data } => {
            Some(CanonicalContent::Image(ImageSource::Base64 {
                media_type: media_type.unwrap_or_else(|| "image/png".to_owned()),
                data,
            }))
        },
        AnthropicImageSource::Url { url } => Some(CanonicalContent::Image(ImageSource::Url(url))),
        AnthropicImageSource::Unknown => None,
    }
}

/// `msg_id` carries the message id observed at `message_start` so later
/// `message_stop` frames can be tagged. Returns `None` for frames the canonical
/// model does not model (e.g. `ping`).
#[must_use]
pub fn event_from_sse(value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "message_start" => convert_message_start(value),
        "content_block_start" => convert_content_block_start(value),
        "content_block_delta" => convert_content_block_delta(value),
        "content_block_stop" => Some(CanonicalEvent::ContentBlockStop {
            index: u32_field(value, "index"),
        }),
        "message_delta" => convert_message_delta(value, msg_id),
        "message_stop" => Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason: None,
        }),
        "error" => Some(CanonicalEvent::Error(
            value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream error")
                .to_owned(),
        )),
        _ => None,
    }
}

fn convert_message_start(value: &Value) -> Option<CanonicalEvent> {
    let msg = value.get("message")?;
    Some(CanonicalEvent::MessageStart {
        id: str_field(msg, "id", ""),
        model: str_field(msg, "model", ""),
        usage: usage_from_value(msg.get("usage")),
    })
}

fn convert_content_block_start(value: &Value) -> Option<CanonicalEvent> {
    let index = u32_field(value, "index");
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
            id: str_field(block, "id", ""),
            name: str_field(block, "name", ""),
        },
        _ => return None,
    };
    Some(CanonicalEvent::ContentBlockStart { index, block: kind })
}

fn convert_content_block_delta(value: &Value) -> Option<CanonicalEvent> {
    let index = u32_field(value, "index");
    let delta = value.get("delta")?;
    let dtype = delta.get("type").and_then(Value::as_str)?;
    let text_field = |field: &str| str_field(delta, field, "");
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
    if stop_reason.is_some() {
        return Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason,
        });
    }
    value
        .get("usage")
        .map(|u| CanonicalEvent::UsageDelta(usage_from_value(Some(u))))
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

fn str_field(value: &Value, field: &str, default: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn u32_field(value: &Value, field: &str) -> u32 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0) as u32
}
