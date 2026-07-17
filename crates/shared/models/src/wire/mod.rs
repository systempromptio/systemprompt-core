//! Canonical AI wire types and per-protocol codecs, shared by the gateway and
//! the agent provider clients.
//!
//! The gateway speaks one provider-neutral model internally. Inbound adapters
//! parse a client wire request into a [`canonical::CanonicalRequest`]; outbound
//! adapters render that request to an upstream provider, parse the upstream
//! reply into a [`canonical::CanonicalResponse`], and map upstream SSE bytes to
//! [`canonical::CanonicalEvent`]s.
//!
//! - [`canonical`] holds those provider-neutral request/response/event types.
//! - The per-protocol modules ([`anthropic`], [`openai_chat`],
//!   [`openai_responses`], [`gemini`]) hold the codec for one wire dialect:
//!   request build, response parse, stop-reason + usage mapping, SSE-to-event
//!   translation, and auth-header construction.
//!
//! These types are defined ONCE here and re-exported by the gateway and the
//! agent provider clients so both layers share a single wire vocabulary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod canonical;

pub mod anthropic;
pub mod gemini;
pub mod openai_chat;
pub mod openai_responses;
pub mod sse;

#[must_use]
pub fn clamp_output_tokens(requested: u32, max_output_tokens: Option<u32>) -> u32 {
    match max_output_tokens {
        Some(cap) if cap > 0 => requested.min(cap),
        _ => requested,
    }
}
