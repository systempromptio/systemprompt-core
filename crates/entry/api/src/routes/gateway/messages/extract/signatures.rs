//! Whether a request can reach an upstream that needs Gemini thought
//! signatures.
//!
//! Every hop the resolved chain may be served by counts: the primary, its
//! failover deployments and the context fallbacks. One `gemini`-wire hop
//! anywhere in that set makes hydration required, so a failover hop still
//! carries the signatures it needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_gateway::RouteResolution;
use systemprompt_wire::WireProtocol;

#[must_use]
pub fn route_needs_thought_signatures(resolution: &RouteResolution) -> bool {
    resolution
        .target
        .chain
        .iter()
        .chain(&resolution.target.context_fallbacks)
        .any(|hop| hop.wire == WireProtocol::Gemini)
}
