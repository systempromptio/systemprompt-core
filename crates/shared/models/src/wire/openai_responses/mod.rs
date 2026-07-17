//! `OpenAI` Responses wire codec.
//!
//! Builds an `OpenAI` Responses upstream request from a
//! [`crate::wire::canonical::CanonicalRequest`], parses the buffered reply into
//! a [`crate::wire::canonical::CanonicalResponse`], and maps Responses SSE
//! bytes to [`crate::wire::canonical::CanonicalEvent`]s.
//!
//! Every public function here is a pure codec over [`serde_json::Value`] or a
//! byte stream; transport, auth, and HTTP status handling stay in the gateway
//! adapter that calls these. The `request` submodule owns the request build,
//! `response` the buffered-reply parse, `streaming` the SSE-to-event pipeline,
//! and `slot` the per-output-item slot state machine the streaming pass tracks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;
mod slot;
mod streaming;

pub use request::build_request_body;
pub use response::parse_response_object;
pub use streaming::sse_to_canonical_events;
