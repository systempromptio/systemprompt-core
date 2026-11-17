//! Programmatic refinement of the gateway's declaratively-matched route.
//!
//! The profile's `gateway.routes` select a backend by model glob and optional
//! [`RouteMatch`](systemprompt_manifest::services::RouteMatch) predicates. When
//! a request needs classification those predicates cannot express — accurate
//! token counting, a learned task classifier, heuristic shape detection — a
//! [`RouteSelector`] contributed to
//! [`RouteSelectorSlot`](crate::contract::RouteSelectorSlot) may *re-route*
//! the request to a different
//! [`GatewayRoute`]. It can never deny: the authorization hook owns denial,
//! and a selector that errors is logged and treated as no-op so a
//! misconfigured extension can never fail dispatch.
//!
//! This mirrors the
//! [`SystemPromptOverride`](super::overrides::SystemPromptOverride)
//! seam: a `dyn`-object trait, a core-first slot, and one engine built from
//! the admitted entries. Selectors run core first, then installs in
//! composition order; the first that re-routes wins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_wire::canonical::CanonicalRequest;

#[derive(Debug, thiserror::Error)]
pub enum RouteSelectorError {
    #[error("route selector '{name}' failed: {message}")]
    Failed { name: &'static str, message: String },
}

/// A programmatic re-router consulted after declarative route matching.
///
/// `#[async_trait]` is required: selectors are held as trait objects in the
/// engine registry, so the trait must stay `dyn`-compatible. `refine` returns
/// `Some(route)` to replace the matched route, or `None` to keep it. A
/// selector re-routes only — it cannot deny a request.
#[async_trait]
pub trait RouteSelector: Send + Sync {
    fn name(&self) -> &'static str;
    async fn refine(
        &self,
        matched: &GatewayRoute,
        request: &CanonicalRequest,
    ) -> Result<Option<GatewayRoute>, RouteSelectorError>;
}

pub struct RouteSelectorChain {
    selectors: Vec<Arc<dyn RouteSelector>>,
}

impl Default for RouteSelectorChain {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl std::fmt::Debug for RouteSelectorChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSelectorChain")
            .field("selectors", &self.selectors.len())
            .finish()
    }
}

impl RouteSelectorChain {
    #[must_use]
    pub fn new(selectors: Vec<Arc<dyn RouteSelector>>) -> Self {
        Self { selectors }
    }

    #[must_use]
    pub fn has_selectors(&self) -> bool {
        !self.selectors.is_empty()
    }

    pub async fn refine(
        &self,
        matched: &GatewayRoute,
        request: &CanonicalRequest,
    ) -> Option<(GatewayRoute, &'static str)> {
        for selector in &self.selectors {
            match selector.refine(matched, request).await {
                Ok(Some(route)) => {
                    tracing::info!(
                        selector = selector.name(),
                        from_route = %matched.id,
                        to_route = %route.id,
                        "gateway route refined by selector"
                    );
                    return Some((route, selector.name()));
                },
                Ok(None) => {},
                Err(e) => {
                    tracing::warn!(
                        selector = selector.name(),
                        error = %e,
                        "route selector errored; keeping matched route"
                    );
                },
            }
        }
        None
    }
}
