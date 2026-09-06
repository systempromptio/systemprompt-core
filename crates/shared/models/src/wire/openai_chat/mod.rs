//! `OpenAI` Chat Completions wire codec.
//!
//! Builds an `OpenAI` Chat upstream request from a
//! [`crate::wire::canonical::CanonicalRequest`], parses the buffered reply into
//! a [`crate::wire::canonical::CanonicalResponse`], and maps SSE bytes to a
//! stream of [`crate::wire::canonical::CanonicalEvent`]s. Also serves
//! OpenAI-compatible providers exposing the same surface. Auth-header and
//! transport concerns stay with the gateway adapter; this module is pure wire
//! translation.
//!
//! Reasoning models bill internal reasoning from the same completion budget as
//! visible output, so a caller `max_tokens` — which on the inbound Anthropic
//! surface bounds only visible output — can be consumed entirely by reasoning
//! and trigger an upstream output-limit rejection. `output_token_ceiling`
//! therefore uses the full model-card cap as the budget for these families;
//! `is_reasoning_model` identifies them, either from the model card's
//! `limits.max_thinking_budget` or, for families that reason without one, from
//! a name prefix (`gpt-5*`, `o1*`, `o3*`, `o4*`). For
//! every other model it clamps the caller's `max_tokens` *down* to the cap when
//! one is known (never raising it) — keeping the upstream within the model's
//! real output limit and giving operators a per-request TPM lever via the
//! model card's `limits.max_output_tokens`. Both `OpenAI` codecs (Chat
//! Completions and Responses) share these.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;
mod stream_delta;
mod streaming;

pub use request::build_request_body;
pub use response::{buffered_defect, parse_response};
pub use streaming::sse_to_canonical_events;

use crate::services::ai::ModelLimits;
use crate::wire::canonical::CanonicalRequest;

// Why: the model card is the authority -- a non-zero `max_thinking_budget`
// means the provider bills thought tokens against the completion budget,
// which is what the ceiling has to compensate for. The prefix list stays for
// the `OpenAI` families that reason without carrying a catalog budget.
pub(crate) fn is_reasoning_model(model: &str, limits: Option<ModelLimits>) -> bool {
    const REASONING_PREFIXES: [&str; 4] = ["gpt-5", "o1", "o3", "o4"];
    if limits
        .and_then(|l| l.max_thinking_budget)
        .is_some_and(|b| b > 0)
    {
        return true;
    }
    REASONING_PREFIXES
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

pub(crate) fn output_token_ceiling(
    request: &CanonicalRequest,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> u32 {
    passthrough_output_tokens(request.max_tokens, upstream_model, limits)
}

// Why: the byte-passthrough lanes forward the caller's own body rather than
// building one, so they need the identical budget arithmetic keyed on the raw
// token field they read. Without it passthrough is a way around the model
// card -- a caller limit above the cap reaches the upstream as a hard 400,
// and a reasoning model spends the caller's whole budget on thought.
#[must_use]
pub fn passthrough_output_tokens(
    requested: u32,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> u32 {
    let max_output_tokens = limits.map(|l| l.max_output_tokens);
    match max_output_tokens {
        Some(cap) if cap > 0 && is_reasoning_model(upstream_model, limits) => cap,
        _ => super::clamp_output_tokens(requested, max_output_tokens),
    }
}
