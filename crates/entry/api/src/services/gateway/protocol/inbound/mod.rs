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

// Why: the wire contract names a client mistake `invalid_request_error`; the
// generic `api_error` is reserved for the server's own faults, so a rendered
// error must take its type from the status it is being sent with.
pub(crate) fn error_type_for_status(status: StatusCode) -> &'static str {
    match status {
        StatusCode::UNAUTHORIZED => "authentication_error",
        StatusCode::FORBIDDEN => "permission_error",
        StatusCode::NOT_FOUND => "not_found_error",
        StatusCode::TOO_MANY_REQUESTS => "rate_limit_error",
        s if s.is_client_error() => "invalid_request_error",
        _ => "api_error",
    }
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
        _event: &CanonicalEvent,
        _snapshot: &CanonicalResponse,
        _model: &str,
    ) -> Option<Bytes> {
        None
    }

    fn wants_stream_usage(&self, _raw: &Bytes) -> bool {
        false
    }

    fn render_stream_tail(
        &self,
        _snapshot: &CanonicalResponse,
        _include_usage: bool,
    ) -> Option<Bytes> {
        None
    }

    fn render_error(&self, status: StatusCode, message: &str) -> Bytes;
    fn streaming_content_type(&self) -> &'static str {
        "text/event-stream"
    }
}
