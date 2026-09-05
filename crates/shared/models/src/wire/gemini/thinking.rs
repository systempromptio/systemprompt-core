//! Thinking budget arithmetic for the Gemini `generateContent` request body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::wire::GeminiThinkingConfig;
use crate::services::ai::ModelLimits;
use crate::wire::canonical::CanonicalRequest;

// Why: Gemini 2.5 thinks by default and bills thought tokens against
// maxOutputTokens, so a caller's `max_tokens` can be spent entirely on thinking
// and return empty text. With no client thinking block we budget thinking *on
// top of* the caller's text budget -- budget = min(max_thinking_budget,
// cap - text), maxOutputTokens = text + budget -- so both fit under the cap.
// An explicit client block keeps the caller's own maxOutputTokens.
pub(super) fn thinking_config(
    request: &CanonicalRequest,
    limits: Option<ModelLimits>,
) -> (Option<GeminiThinkingConfig>, u32) {
    let cap = limits.map(|l| l.max_output_tokens);
    let text = crate::wire::clamp_output_tokens(request.max_tokens, cap);
    let max_budget = limits.and_then(|l| l.max_thinking_budget);
    let Some(thinking) = request.thinking else {
        return (
            implicit(text, cap, max_budget),
            implicit_ceiling(text, cap, max_budget),
        );
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

// Why: Gemini 2.5 Pro rejects a non-zero thinkingBudget below 128 and cannot
// have thinking disabled, so a budget derived from the room left under the
// model cap has to be floored rather than sent as a smaller number.
const MIN_THINKING_BUDGET: u32 = 128;

// Why: prefer the room left under the model cap, but never dip below the
// floor -- when the caller's text budget leaves less than that, the floor is
// taken out of the text budget instead, which `implicit_ceiling` caps.
fn implicit_budget(text: u32, cap: Option<u32>, max_budget: Option<u32>) -> u32 {
    let room = cap
        .filter(|c| *c > 0)
        .map_or(u32::MAX, |c| c.saturating_sub(text));
    let allowed = max_budget.unwrap_or(0);
    if allowed == 0 {
        return 0;
    }
    allowed.min(room.max(MIN_THINKING_BUDGET))
}

fn implicit(text: u32, cap: Option<u32>, max_budget: Option<u32>) -> Option<GeminiThinkingConfig> {
    match implicit_budget(text, cap, max_budget) {
        0 => None,
        budget => Some(config(Some(budget))),
    }
}

// Why: the floor can push text + budget past the model cap; clamping here is
// what makes the floor come out of the text budget rather than breach the cap.
fn implicit_ceiling(text: u32, cap: Option<u32>, max_budget: Option<u32>) -> u32 {
    let ceiling = text.saturating_add(implicit_budget(text, cap, max_budget));
    crate::wire::clamp_output_tokens(ceiling, cap)
}

const fn config(thinking_budget: Option<u32>) -> GeminiThinkingConfig {
    GeminiThinkingConfig {
        thinking_budget,
        include_thoughts: Some(true),
    }
}
