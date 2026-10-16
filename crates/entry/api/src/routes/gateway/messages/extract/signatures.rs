//! Whether a route can reach an upstream that needs Gemini thought signatures.
//!
//! Every deployment the route may be served by counts: the primary, its
//! fallbacks and context fallbacks, and every per-scope chain. One Gemini
//! deployment anywhere in that set makes hydration required, so a fallback
//! hop still carries the signatures it needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::iter;

use systemprompt_identifiers::ProviderId;
use systemprompt_manifest::services::ProviderRegistry;
use systemprompt_manifest::services::gateway::GatewayRoute;
use systemprompt_wire::WireProtocol;

pub fn route_needs_thought_signatures(providers: &ProviderRegistry, route: &GatewayRoute) -> bool {
    let scoped = route
        .by_scope
        .iter()
        .flat_map(|scoped| scoped.chains.values())
        .flat_map(|chain| {
            iter::once(&chain.provider).chain(
                chain
                    .fallbacks
                    .iter()
                    .chain(&chain.context_fallbacks)
                    .map(|d| &d.provider),
            )
        });
    iter::once(&route.provider)
        .chain(
            route
                .fallbacks
                .iter()
                .chain(&route.context_fallbacks)
                .map(|d| &d.provider),
        )
        .chain(scoped)
        .any(|provider: &ProviderId| {
            providers
                .find_provider(provider.as_str())
                .is_some_and(|entry| entry.wire == WireProtocol::Gemini)
        })
}
