//! `OpenAI` Responses buffered-reply parse: Responses object → canonical
//! response.

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

#[derive(Debug, Default, Deserialize)]
struct ResponseObject {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<ResponseUsage>,
    #[serde(default)]
    output: Vec<OutputItem>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ResponseUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<MessagePart>,
    },
    FunctionCall(FunctionCall),
    Reasoning {
        #[serde(default)]
        summary: Vec<SummaryPart>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct MessagePart {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct SummaryPart {
    #[serde(default)]
    text: Option<String>,
}

pub fn parse_response_object(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let resp = ResponseObject::deserialize(value).unwrap_or_default();
    let id = resp
        .id
        .unwrap_or_else(|| format!("resp_{}", Uuid::new_v4().simple()));
    let model = resp.model.unwrap_or_else(|| fallback_model.to_owned());
    let usage = resp.usage.map_or(
        CanonicalUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
        |u| CanonicalUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        },
    );

    let mut content: Vec<CanonicalContent> = Vec::new();
    for item in resp.output {
        collect_output_item(item, &mut content);
    }

    let stop_reason = resp
        .stop_reason
        .as_deref()
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

fn collect_output_item(item: OutputItem, content: &mut Vec<CanonicalContent>) {
    match item {
        OutputItem::Message { content: parts } => {
            for part in parts {
                if matches!(part.kind.as_str(), "output_text" | "text") {
                    if let Some(text) = part.text {
                        content.push(CanonicalContent::Text(text));
                    }
                }
            }
        },
        OutputItem::FunctionCall(call) => {
            let id = call.call_id.or(call.id).unwrap_or_default();
            let args = if call.arguments.is_empty() {
                "{}"
            } else {
                &call.arguments
            };
            // Tool-call arguments are a user-defined schema instance; the
            // canonical model carries them as an opaque JSON value.
            let input: Value = serde_json::from_str(args)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            content.push(CanonicalContent::ToolUse {
                id,
                name: call.name,
                input,
            });
        },
        OutputItem::Reasoning { summary } => {
            let text = summary
                .into_iter()
                .filter_map(|s| s.text)
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                content.push(CanonicalContent::Thinking {
                    text,
                    signature: None,
                });
            }
        },
        OutputItem::Unknown => {},
    }
}
