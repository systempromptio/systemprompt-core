//! Inbound adapter for the `OpenAI` Chat Completions wire protocol.
//!
//! [`OpenAiChatInbound`] parses Chat Completions request bodies into the
//! canonical request model and renders canonical responses, streaming chunks,
//! and errors back in Chat Completions format. This is the surface `OpenCode`,
//! VS Code Copilot BYOK, and other OpenAI-SDK clients speak.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use http::StatusCode;
use serde_json::Value;
use systemprompt_models::services::WireProtocol;

use super::super::canonical::CanonicalRequest;
use super::super::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::{InboundAdapter, InboundParseError};

mod parse;
mod render;
mod render_terminal;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::parse::parse as parse_request;
    pub use super::render::{render_event_frame, render_response_object};
    pub use super::render_terminal::render_terminal_event_frame;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiChatInbound;

impl InboundAdapter for OpenAiChatInbound {
    fn wire_name(&self) -> &'static str {
        "openai.chat"
    }

    fn passthrough_wire(&self) -> Option<WireProtocol> {
        Some(WireProtocol::OpenAiChat)
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

    fn wants_stream_usage(&self, raw: &Bytes) -> bool {
        serde_json::from_slice::<Value>(raw).is_ok_and(|v| {
            v.get("stream_options")
                .and_then(|o| o.get("include_usage"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
    }

    fn render_stream_tail(
        &self,
        snapshot: &CanonicalResponse,
        include_usage: bool,
    ) -> Option<Bytes> {
        Some(render_terminal::render_stream_tail_frames(
            snapshot,
            include_usage,
        ))
    }

    fn render_error(&self, _status: StatusCode, message: &str) -> Bytes {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let body = format!("{{\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}");
        Bytes::from(body)
    }
}
