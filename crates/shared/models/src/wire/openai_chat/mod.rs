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
//! Reasoning models (`gpt-5*`, `o1*`, `o3*`, `o4*`) bill internal reasoning
//! from the same completion budget as visible output, so a caller `max_tokens`
//! — which on the inbound Anthropic surface bounds only visible output — can be
//! consumed entirely by reasoning and trigger an upstream output-limit
//! rejection. `output_token_ceiling` therefore uses the full model-card cap as
//! the budget for these families; `is_reasoning_model` identifies them. For
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
mod streaming;

pub use request::build_request_body;
pub use response::parse_response;
pub use streaming::sse_to_canonical_events;

use crate::services::ai::ModelLimits;
use crate::wire::canonical::CanonicalRequest;

pub(crate) fn is_reasoning_model(model: &str) -> bool {
    const REASONING_PREFIXES: [&str; 4] = ["gpt-5", "o1", "o3", "o4"];
    REASONING_PREFIXES
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

pub(crate) fn output_token_ceiling(
    request: &CanonicalRequest,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> u32 {
    let max_output_tokens = limits.map(|l| l.max_output_tokens);
    match max_output_tokens {
        Some(cap) if cap > 0 && is_reasoning_model(upstream_model) => cap,
        _ => super::clamp_output_tokens(request.max_tokens, max_output_tokens),
    }
}
