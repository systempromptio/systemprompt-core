//! Anthropic Messages response + SSE-frame parse side of the codec.
//!
//! [`parse_response`] deserializes a buffered Messages reply into a
//! [`CanonicalResponse`]; [`event_from_sse`] turns one decoded SSE `data:`
//! payload into a [`CanonicalEvent`]. The streaming side stays dynamic because
//! each frame is a distinct, sparsely-populated event keyed on `type`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::Value;

use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, GroundedSource,
    Grounding, ImageSource,
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
#[expect(
    clippy::struct_field_names,
    reason = "field names mirror the Anthropic usage wire schema verbatim"
)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
}

impl AnthropicUsage {
    const fn into_canonical(self) -> CanonicalUsage {
        CanonicalUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.cache_read_input_tokens,
            cache_creation_tokens: self.cache_creation_input_tokens,
            total_tokens: self.input_tokens
                + self.output_tokens
                + self.cache_read_input_tokens
                + self.cache_creation_input_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    Text {
        #[serde(default)]
        text: String,
        #[serde(default)]
        citations: Vec<AnthropicCitation>,
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
        #[serde(default)]
        signature: Option<String>,
    },
    Image {
        source: AnthropicImageSource,
    },
    WebSearchToolResult {
        #[serde(default)]
        content: Vec<AnthropicWebSearchResult>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct AnthropicWebSearchResult {
    #[serde(default)]
    url: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicCitation {
    #[serde(default)]
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cited_text: Option<String>,
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
        .map(AnthropicUsage::into_canonical)
        .unwrap_or_default();

    let mut content = Vec::with_capacity(resp.content.len());
    let mut sources: Vec<GroundedSource> = Vec::new();
    for block in resp.content {
        match block {
            AnthropicBlock::WebSearchToolResult { content: results } => {
                sources.extend(results.into_iter().filter(|r| !r.url.is_empty()).map(|r| {
                    GroundedSource {
                        uri: r.url,
                        title: r.title,
                        ..GroundedSource::default()
                    }
                }));
            },
            AnthropicBlock::Text { text, citations } => {
                for c in citations.into_iter().filter(|c| !c.url.is_empty()) {
                    sources.push(GroundedSource {
                        uri: c.url,
                        title: c.title,
                        snippet: c.cited_text,
                        relevance: None,
                    });
                }
                content.push(CanonicalContent::Text(text));
            },
            other => {
                if let Some(part) = canonical_block(other) {
                    content.push(part);
                }
            },
        }
    }
    let grounding = (!sources.is_empty()).then(|| Grounding {
        sources,
        queries: Vec::new(),
    });

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
        grounding,
        code_execution: None,
        raw_finish_reason: resp.stop_reason,
    }
}

fn canonical_block(block: AnthropicBlock) -> Option<CanonicalContent> {
    match block {
        AnthropicBlock::Text { text, .. } => Some(CanonicalContent::Text(text)),
        AnthropicBlock::Thinking {
            thinking,
            signature,
        } => Some(CanonicalContent::Thinking {
            text: thinking,
            signature,
        }),
        AnthropicBlock::ToolUse {
            id,
            name,
            input,
            signature,
        } => Some(CanonicalContent::ToolUse {
            id,
            name,
            input,
            signature,
        }),
        AnthropicBlock::Image { source } => canonical_image(source),
        AnthropicBlock::WebSearchToolResult { .. } | AnthropicBlock::Unknown => None,
    }
}

fn canonical_image(source: AnthropicImageSource) -> Option<CanonicalContent> {
    match source {
        AnthropicImageSource::Base64 { media_type, data } => {
            Some(CanonicalContent::Image(ImageSource::Base64 {
                media_type: media_type.unwrap_or_else(|| "image/png".to_owned()),
                data,
                detail: None,
            }))
        },
        AnthropicImageSource::Url { url } => Some(CanonicalContent::Image(ImageSource::Url {
            url,
            detail: None,
        })),
        AnthropicImageSource::Unknown => None,
    }
}
