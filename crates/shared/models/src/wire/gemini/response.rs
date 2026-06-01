//! Parses a buffered Gemini reply into a [`CanonicalResponse`].

use serde_json::Value;
use uuid::Uuid;

use super::wire::{GeminiPart, GeminiResponse, GeminiUsageMetadata};
use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

#[must_use]
pub fn stop_reason(finish: &str) -> CanonicalStopReason {
    match finish {
        "STOP" => CanonicalStopReason::EndTurn,
        "MAX_TOKENS" => CanonicalStopReason::MaxTokens,
        _ => CanonicalStopReason::Other,
    }
}

/// Falls back to `fallback_model` when the upstream omits `modelVersion`.
#[must_use]
pub fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let parsed: GeminiResponse = serde_json::from_value(value.clone()).unwrap_or(GeminiResponse {
        candidates: Vec::new(),
        usage_metadata: None,
        response_id: None,
        model_version: None,
    });

    let id = parsed
        .response_id
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));
    let model = parsed
        .model_version
        .unwrap_or_else(|| fallback_model.to_owned());

    let candidate = parsed.candidates.into_iter().next();
    let stop = candidate
        .as_ref()
        .and_then(|c| c.finish_reason.as_deref())
        .map(stop_reason);
    let content = candidate
        .and_then(|c| c.content)
        .map(|c| parts_to_content(&c.parts))
        .unwrap_or_default();

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason: stop,
        usage: usage(parsed.usage_metadata),
    }
}

fn usage(meta: Option<GeminiUsageMetadata>) -> CanonicalUsage {
    meta.map_or_else(CanonicalUsage::default, |u| CanonicalUsage {
        input_tokens: u.prompt,
        output_tokens: u.candidates,
    })
}

/// Tool-use blocks get freshly minted ids because Gemini omits them on the
/// wire.
pub(super) fn parts_to_content(parts: &[GeminiPart]) -> Vec<CanonicalContent> {
    parts
        .iter()
        .filter_map(|part| match part {
            GeminiPart::Text { text } if !text.is_empty() => {
                Some(CanonicalContent::Text(text.clone()))
            },
            GeminiPart::FunctionCall { function_call } => Some(CanonicalContent::ToolUse {
                id: format!("call_{}", Uuid::new_v4().simple()),
                name: function_call.name.clone(),
                input: function_call.args.clone(),
            }),
            _ => None,
        })
        .collect()
}
