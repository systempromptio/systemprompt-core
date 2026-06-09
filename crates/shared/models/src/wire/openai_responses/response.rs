//! `OpenAI` Responses buffered-reply parse: Responses object → canonical
//! response.

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, GroundedSource,
    Grounding,
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
    incomplete_details: Option<IncompleteDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct IncompleteDetails {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ResponseUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
    #[serde(default)]
    input_tokens_details: ResponseInputTokensDetails,
}

#[derive(Debug, Default, Deserialize)]
struct ResponseInputTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

impl ResponseUsage {
    const fn into_canonical(self) -> CanonicalUsage {
        CanonicalUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.input_tokens_details.cached_tokens,
            cache_creation_tokens: 0,
            total_tokens: self.total_tokens,
        }
    }
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
    #[serde(default)]
    annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
struct Annotation {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
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
    let usage = resp
        .usage
        .map(ResponseUsage::into_canonical)
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    let mut sources: Vec<GroundedSource> = Vec::new();
    for item in resp.output {
        collect_output_item(item, &mut content, &mut sources);
    }
    let grounding = (!sources.is_empty()).then(|| Grounding {
        sources,
        queries: Vec::new(),
    });

    let incomplete_reason = resp.incomplete_details.and_then(|d| d.reason);
    let stop_reason = Some(buffered_stop_reason(&content, incomplete_reason.as_deref()));

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
        grounding,
        code_execution: None,
        raw_finish_reason: incomplete_reason,
    }
}

// Responses has no finish-reason field: tool use is signalled by a
// `function_call` output item, truncation by `incomplete_details.reason`.
fn buffered_stop_reason(
    content: &[CanonicalContent],
    incomplete_reason: Option<&str>,
) -> CanonicalStopReason {
    if content
        .iter()
        .any(|c| matches!(c, CanonicalContent::ToolUse { .. }))
    {
        return CanonicalStopReason::ToolUse;
    }
    match incomplete_reason {
        Some("max_output_tokens") => CanonicalStopReason::MaxTokens,
        Some(_) => CanonicalStopReason::Other,
        None => CanonicalStopReason::EndTurn,
    }
}

fn collect_output_item(
    item: OutputItem,
    content: &mut Vec<CanonicalContent>,
    sources: &mut Vec<GroundedSource>,
) {
    match item {
        OutputItem::Message { content: parts } => {
            for part in parts {
                for a in part.annotations {
                    if let Some(uri) = a.url.filter(|u| !u.is_empty()) {
                        sources.push(GroundedSource {
                            uri,
                            title: a.title,
                            ..GroundedSource::default()
                        });
                    }
                }
                if matches!(part.kind.as_str(), "output_text" | "text")
                    && let Some(text) = part.text
                {
                    content.push(CanonicalContent::Text(text));
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
                signature: None,
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
