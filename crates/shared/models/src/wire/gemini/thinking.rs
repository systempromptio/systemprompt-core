//! Thinking budget arithmetic for the Gemini `generateContent` request body.
//!
//! Gemini bills thought tokens against `maxOutputTokens`, so a caller's
//! `max_tokens` -- which bounds visible text on the inbound surface -- can be
//! spent entirely on thinking and return empty text. With no client thinking
//! block we therefore raise the ceiling to text + `max_thinking_budget`,
//! clamped to the model cap, and send no `thinkingConfig` at all: a
//! `thinkingBudget` would switch thinking on for models Google ships with it
//! off (Flash-Lite) or widen it for models whose default is dynamic, which is
//! a behaviour change the caller never asked for. An explicit client block
//! keeps the caller's own ceiling and its own budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::wire::GeminiThinkingConfig;
use crate::services::ai::ModelLimits;
use crate::wire::canonical::CanonicalRequest;

pub(super) fn thinking_config(
    request: &CanonicalRequest,
    limits: Option<ModelLimits>,
) -> (Option<GeminiThinkingConfig>, u32) {
    let cap = limits.map(|l| l.max_output_tokens);
    let text = crate::wire::clamp_output_tokens(request.max_tokens, cap);
    let max_budget = limits.and_then(|l| l.max_thinking_budget);
    let Some(thinking) = request.thinking else {
        return (None, headroom_ceiling(text, cap, max_budget));
    };
    if !thinking.enabled {
        return (None, text);
    }
    let budget = match (thinking.budget_tokens, max_budget) {
        (Some(want), Some(cap)) => Some(want.min(cap)),
        (want, _) => want,
    };
    (Some(config(budget)), text)
}

// Why: room for the model's own default thinking on top of the caller's text
// budget, never past the model's output cap. A model card with no thinking
// budget leaves the caller's number exactly as it was.
fn headroom_ceiling(text: u32, cap: Option<u32>, max_budget: Option<u32>) -> u32 {
    let ceiling = text.saturating_add(max_budget.unwrap_or(0));
    crate::wire::clamp_output_tokens(ceiling, cap)
}

const fn config(thinking_budget: Option<u32>) -> GeminiThinkingConfig {
    GeminiThinkingConfig {
        thinking_budget,
        include_thoughts: Some(true),
    }
}
