//! Gemini `generateContent` / `streamGenerateContent` wire codec.
//!
//! Builds a Google generativeLanguage v1beta request from a
//! [`crate::wire::canonical::CanonicalRequest`], parses the buffered reply into
//! a [`crate::wire::canonical::CanonicalResponse`], and maps the SSE byte
//! stream (`?alt=sse`) to [`crate::wire::canonical::CanonicalEvent`]s.
//!
//! Gemini authenticates with an `x-goog-api-key` header (the `?key=` query
//! param is the alternative; this codec uses the header so keys stay out of
//! request lines and logs). The wire shapes are kept private to this module so
//! the shared wire codec stays free of the agent-side `domain/ai` crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;
mod streaming;
mod wire;

pub use request::build_request_body;
pub use response::{parse_response, stop_reason};
pub use streaming::sse_to_canonical_events;

pub const API_KEY_HEADER: &str = "x-goog-api-key";

#[must_use]
pub fn upstream_path(model: &str, stream: bool) -> String {
    if stream {
        format!("/models/{model}:streamGenerateContent?alt=sse")
    } else {
        format!("/models/{model}:generateContent")
    }
}
