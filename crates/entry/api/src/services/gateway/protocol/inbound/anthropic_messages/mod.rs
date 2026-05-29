//! Inbound adapter for the Anthropic Messages wire protocol.
//!
//! [`AnthropicMessagesInbound`] parses Messages-format request bodies into the
//! canonical request model and renders canonical responses, streaming events,
//! and errors back in Messages format.

use bytes::Bytes;
use http::StatusCode;
use serde_json::Value;

use super::super::canonical::CanonicalRequest;
use super::super::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::{InboundAdapter, InboundParseError};

mod parse;
mod render;

pub use render::content_to_anthropic_block;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::parse::parse as parse_request;
    pub use super::render::{render_event_frame, render_response_value};
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicMessagesInbound;

impl InboundAdapter for AnthropicMessagesInbound {
    fn wire_name(&self) -> &'static str {
        "anthropic.messages"
    }

    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        let value: Value = serde_json::from_slice(raw)
            .map_err(|e| InboundParseError::InvalidJson(e.to_string()))?;
        parse::parse(&value)
    }

    fn render_response(&self, response: &CanonicalResponse) -> Bytes {
        let value = render::render_response_value(response);
        Bytes::from(serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()))
    }

    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes> {
        render::render_event_frame(event, model)
    }

    fn render_error(&self, _status: StatusCode, message: &str) -> Bytes {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!(
            "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
        );
        Bytes::from(body)
    }
}
