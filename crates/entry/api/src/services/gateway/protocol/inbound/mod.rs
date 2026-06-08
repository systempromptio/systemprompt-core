//! Inbound protocol adapters: caller wire format to canonical model.
//!
//! The [`InboundAdapter`] trait parses a request body into a
//! [`CanonicalRequest`] and renders canonical responses, streaming events, and
//! errors back in the caller's protocol. Implementations cover the Anthropic
//! Messages and `OpenAI` Responses surfaces; [`InboundParseError`] reports
//! malformed or unsupported inputs.

pub mod anthropic_messages;
pub mod openai_responses;

use bytes::Bytes;
use http::StatusCode;

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
    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError>;
    fn render_response(&self, response: &CanonicalResponse) -> Bytes;
    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes>;

    /// Render a terminal streaming event whose wire form must embed
    /// fully-accumulated item content — the complete tool-call arguments and
    /// the output list — which the per-event [`CanonicalEvent`] alone does
    /// not carry. Returns `None` for wires that finalize correctly from
    /// per-event deltas (the caller then falls back to
    /// [`InboundAdapter::render_event`]).
    fn render_terminal_event(
        &self,
        event: &CanonicalEvent,
        snapshot: &CanonicalResponse,
        model: &str,
    ) -> Option<Bytes> {
        let _ = (event, snapshot, model);
        None
    }

    fn render_error(&self, status: StatusCode, message: &str) -> Bytes;
    fn streaming_content_type(&self) -> &'static str {
        "text/event-stream"
    }
}
