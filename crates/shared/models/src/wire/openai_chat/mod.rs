//! `OpenAI` Chat Completions wire codec.
//!
//! Builds an `OpenAI` Chat upstream request from a
//! [`crate::wire::canonical::CanonicalRequest`], parses the buffered reply into
//! a [`crate::wire::canonical::CanonicalResponse`], and maps SSE bytes to a
//! stream of [`crate::wire::canonical::CanonicalEvent`]s. Also serves
//! OpenAI-compatible providers exposing the same surface. Auth-header and
//! transport concerns stay with the gateway adapter; this module is pure wire
//! translation.

mod request;
mod response;
mod streaming;

pub use request::build_request_body;
pub use response::parse_response;
pub use streaming::sse_to_canonical_events;
