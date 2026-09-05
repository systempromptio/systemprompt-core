//! Inbound protocol adapters: caller wire format to canonical model.
//!
//! The [`InboundAdapter`] trait parses a request body into a
//! [`CanonicalRequest`] and renders canonical responses, streaming events, and
//! errors back in the caller's protocol. Implementations cover the Anthropic
//! Messages, `OpenAI` Responses, and `OpenAI` Chat Completions surfaces;
//! [`InboundParseError`] reports malformed or unsupported inputs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anthropic_messages;
pub mod openai_chat;
pub mod openai_responses;

use bytes::Bytes;
use http::StatusCode;
use systemprompt_models::services::WireProtocol;

use super::canonical::CanonicalRequest;
use super::canonical_response::{CanonicalEvent, CanonicalResponse};

#[derive(Debug, thiserror::Error)]
pub enum InboundParseError {
    #[error("invalid request body: {0}")]
    InvalidJson(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("unsupported value for {field}: {detail}")]
    Unsupported { field: &'static str, detail: String },
}

pub trait InboundAdapter: Send + Sync + std::fmt::Debug {
    fn wire_name(&self) -> &'static str;

    fn passthrough_wire(&self) -> Option<WireProtocol> {
        None
    }

    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError>;
    fn render_response(&self, response: &CanonicalResponse) -> Bytes;
    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes>;

    fn render_terminal_event(
        &self,
        event: &CanonicalEvent,
        snapshot: &CanonicalResponse,
        model: &str,
    ) -> Option<Bytes> {
        // Why: unused-arg suppression in a default trait method body.
        let _ = (event, snapshot, model);
        None
    }

    // Why: `stream_options.include_usage` is a Chat Completions concept and
    // the caller's raw body is the only place it is stated, so the surface
    // that understands the field answers for it.
    fn wants_stream_usage(&self, raw: &Bytes) -> bool {
        let _ = raw;
        false
    }

    // Why: the frames that close a streamed turn after its last event. Chat
    // Completions ends with a usage-only chunk and the `[DONE]` sentinel, and
    // both must follow the finish chunk -- the usage a client asked for is
    // only complete once the upstream stream has ended.
    fn render_stream_tail(
        &self,
        snapshot: &CanonicalResponse,
        include_usage: bool,
    ) -> Option<Bytes> {
        let _ = (snapshot, include_usage);
        None
    }

    fn render_error(&self, status: StatusCode, message: &str) -> Bytes;
    fn streaming_content_type(&self) -> &'static str {
        "text/event-stream"
    }
}
