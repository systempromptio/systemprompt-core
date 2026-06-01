//! Parses a buffered Gemini reply into a [`CanonicalResponse`].

use serde_json::Value;
use uuid::Uuid;

use super::wire::{GeminiCandidate, GeminiPart, GeminiResponse, GeminiUsageMetadata};
use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, CodeExecutionOutput,
    GroundedSource, Grounding,
};

// Gemini grounding chunks carry no per-source score; this dialect constant
// stands in so downstream confidence ranking has a stable value.
const GEMINI_GROUNDING_RELEVANCE: f32 = 0.85;

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

    let usage = usage(parsed.usage_metadata);
    let candidate = parsed.candidates.into_iter().next();
    let raw_finish_reason = candidate.as_ref().and_then(|c| c.finish_reason.clone());
    let stop_reason = raw_finish_reason.as_deref().map(stop_reason);
    let grounding = candidate.as_ref().and_then(grounding_from_candidate);
    let parts = candidate.and_then(|c| c.content).map(|c| c.parts);
    let (content, code_execution) = parts.map_or_else(
        || (Vec::new(), None),
        |parts| (parts_to_content(&parts), code_execution(&parts)),
    );

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
        grounding,
        code_execution,
        raw_finish_reason,
    }
}

fn usage(meta: Option<GeminiUsageMetadata>) -> CanonicalUsage {
    meta.map_or_else(CanonicalUsage::default, |u| CanonicalUsage {
        input_tokens: u.prompt,
        output_tokens: u.candidates,
        cache_read_tokens: u.cached,
        cache_creation_tokens: 0,
        total_tokens: if u.total > 0 {
            u.total
        } else {
            u.prompt + u.candidates
        },
    })
}

fn grounding_from_candidate(candidate: &GeminiCandidate) -> Option<Grounding> {
    let meta = candidate.grounding_metadata.as_ref()?;
    let sources: Vec<GroundedSource> = meta
        .grounding_chunks
        .iter()
        .filter_map(|c| c.web.as_ref())
        .filter(|w| !w.uri.is_empty())
        .map(|w| GroundedSource {
            uri: w.uri.clone(),
            title: w.title.clone(),
            relevance: Some(GEMINI_GROUNDING_RELEVANCE),
            ..GroundedSource::default()
        })
        .collect();
    if sources.is_empty() && meta.web_search_queries.is_empty() {
        return None;
    }
    Some(Grounding {
        sources,
        queries: meta.web_search_queries.clone(),
    })
}

fn code_execution(parts: &[GeminiPart]) -> Option<CodeExecutionOutput> {
    let mut output = CodeExecutionOutput::default();
    let mut seen = false;
    for part in parts {
        match part {
            GeminiPart::ExecutableCode { executable_code } => {
                seen = true;
                output.language.clone_from(&executable_code.language);
                output.code.clone_from(&executable_code.code);
            },
            GeminiPart::CodeExecutionResult {
                code_execution_result,
            } => {
                seen = true;
                output.result.clone_from(&code_execution_result.output);
                output.outcome.clone_from(&code_execution_result.outcome);
            },
            _ => {},
        }
    }
    seen.then_some(output)
}

/// Tool-use blocks get freshly minted ids because Gemini omits them on the
/// wire. Executable-code parts are surfaced via `code_execution`, not content.
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
