//! Inbound adapter for the `OpenAI` Responses wire protocol.
//!
//! [`OpenAiResponsesInbound`] parses Responses-format request bodies into the
//! canonical request model and renders canonical responses, streaming events,
//! and errors back in Responses format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use http::StatusCode;
use serde_json::Value;

use super::super::canonical::CanonicalRequest;
use super::super::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::{InboundAdapter, InboundParseError};

mod input;
pub mod parse;
pub mod render;
pub mod render_terminal;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiResponsesInbound;

impl InboundAdapter for OpenAiResponsesInbound {
    fn wire_name(&self) -> &'static str {
        "openai.responses"
    }

    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        let value: Value = serde_json::from_slice(raw)
            .map_err(|e| InboundParseError::InvalidJson(e.to_string()))?;
        parse::parse(&value)
    }

    fn render_response(&self, response: &CanonicalResponse) -> Bytes {
        let value = render::render_response_object(response);
        Bytes::from(serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()))
    }

    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes> {
        render::render_event_frame(event, model)
    }

    fn render_terminal_event(
        &self,
        event: &CanonicalEvent,
        snapshot: &CanonicalResponse,
        _model: &str,
    ) -> Option<Bytes> {
        render_terminal::render_terminal_event_frame(event, snapshot)
    }

    fn render_error(&self, status: StatusCode, message: &str) -> Bytes {
        let kind = super::error_type_for_status(status);
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!("{{\"error\":{{\"type\":\"{kind}\",\"message\":\"{escaped}\"}}}}");
        Bytes::from(body)
    }
}
