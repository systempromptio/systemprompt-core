//! Which upstream wire each inbound request format is.
//!
//! A request served with the credential its client presents reaches the
//! upstream as the client built it, so it can only be served on the wire the
//! client itself spoke. [`spoken_wire`] is that mapping, read by the catalog
//! (a caller credential needs a wire some client can speak) and by the
//! gateway (a caller hop serves a request only on its spoken wire).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::origin::InboundWireProtocol;
use systemprompt_wire::WireProtocol;

#[must_use]
pub const fn spoken_wire(format: InboundWireProtocol) -> Option<WireProtocol> {
    match format {
        InboundWireProtocol::AnthropicMessages => Some(WireProtocol::Anthropic),
        InboundWireProtocol::OpenAiChat => Some(WireProtocol::OpenAiChat),
        InboundWireProtocol::OpenAiResponses => Some(WireProtocol::OpenAiResponses),
        InboundWireProtocol::Internal | InboundWireProtocol::Unknown => None,
    }
}

#[must_use]
pub fn is_spoken_by_a_client(wire: WireProtocol) -> bool {
    InboundWireProtocol::ALL
        .iter()
        .any(|format| spoken_wire(*format) == Some(wire))
}
