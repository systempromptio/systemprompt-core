//! Canonical AI wire types and per-protocol codecs for systemprompt.io,
//! shared by the gateway and the agent provider clients.
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
//! - [`protocol`] names the wire dialect a provider speaks ([`WireProtocol`]),
//!   [`hosting`] the platform in front of it ([`Hosting`]), and [`limits`] the
//!   per-model token ceilings a codec clamps to ([`ModelLimits`]).
//! - [`schema`] holds the per-provider JSON-Schema capability matrices and the
//!   tool-schema sanitiser the codecs apply before sending.
//!
//! These types are defined ONCE here and re-exported by the gateway and the
//! agent provider clients so both layers share a single wire vocabulary. The
//! crate depends only on `systemprompt-identifiers` among the workspace
//! crates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod canonical;
pub mod defect;
pub mod error;

pub mod anthropic;
pub mod gemini;
pub mod hosting;
pub mod inspect;
pub mod limits;
pub mod openai_chat;
pub mod openai_responses;
pub mod protocol;
pub mod schema;
pub mod sse;
pub mod upstream;

pub use hosting::Hosting;
pub use limits::ModelLimits;
pub use protocol::WireProtocol;

#[must_use]
pub fn clamp_output_tokens(requested: u32, max_output_tokens: Option<u32>) -> u32 {
    match max_output_tokens {
        Some(cap) if cap > 0 => requested.min(cap),
        _ => requested,
    }
}
