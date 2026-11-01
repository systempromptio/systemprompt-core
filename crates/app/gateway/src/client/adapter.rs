//! The inbound adapter for a request that is already canonical.
//!
//! An in-process caller hands the gateway a [`CanonicalRequest`] directly, so
//! there is no wire body to parse and no client wire to pass through to:
//! [`CanonicalInbound::passthrough_wire`] is `None`, which makes every upstream
//! body an encoding of the canonical request. Buffered responses render in the
//! Anthropic Messages shape, the one shape [`super::decode`] reads back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use http::StatusCode;
use systemprompt_models::origin::InboundWireProtocol;
use systemprompt_wire::WireProtocol;

use crate::protocol::canonical::{CanonicalEvent, CanonicalRequest, CanonicalResponse};
use crate::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use crate::protocol::inbound::{InboundAdapter, InboundParseError};

/// Inbound side of an in-process gateway call.
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalInbound;

impl InboundAdapter for CanonicalInbound {
    fn wire(&self) -> InboundWireProtocol {
        InboundWireProtocol::Internal
    }

    fn passthrough_wire(&self) -> Option<WireProtocol> {
        None
    }

    fn parse_request(&self, _raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        Err(InboundParseError::MissingField(
            "in-process requests are canonical and carry no wire body",
        ))
    }

    fn render_response(&self, response: &CanonicalResponse) -> Bytes {
        AnthropicMessagesInbound.render_response(response)
    }

    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes> {
        AnthropicMessagesInbound.render_event(event, model)
    }

    fn render_terminal_event(
        &self,
        event: &CanonicalEvent,
        snapshot: &CanonicalResponse,
        model: &str,
    ) -> Option<Bytes> {
        AnthropicMessagesInbound.render_terminal_event(event, snapshot, model)
    }

    fn render_error(&self, status: StatusCode, message: &str) -> Bytes {
        AnthropicMessagesInbound.render_error(status, message)
    }
}
