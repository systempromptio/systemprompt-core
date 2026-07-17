//! Programmatic refinement of the gateway's declaratively-matched route.
//!
//! The profile's `gateway.routes` select a backend by model glob and optional
//! [`RouteMatch`](systemprompt_models::profile::RouteMatch) predicates. When a
//! request needs classification those predicates cannot express — accurate
//! token counting, a learned task classifier, heuristic shape detection — a
//! [`RouteSelector`] contributed through the
//! [`register_route_selector!`](crate::register_route_selector) macro and
//! collected via `inventory` may *re-route* the request to a different
//! [`GatewayRoute`]. It can never deny: the authorization hook owns denial,
//! and a selector that errors is logged and treated as no-op so a
//! misconfigured extension can never fail dispatch.
//!
//! This mirrors the
//! [`SystemPromptOverride`](super::overrides::SystemPromptOverride)
//! seam: a `dyn`-object trait, an `inventory`-collected registration, and a
//! process-global engine resolved once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use systemprompt_models::profile::GatewayRoute;
use systemprompt_models::wire::canonical::CanonicalRequest;

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

#[derive(Debug, Clone, Copy)]
pub struct RouteSelectorRegistration {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn RouteSelector>,
}

inventory::collect!(RouteSelectorRegistration);

/// Register a [`RouteSelector`] implementation with the gateway.
///
/// ```ignore
/// use systemprompt_ai::register_route_selector;
/// register_route_selector!(TokenBudgetSelector::new, name = "token-budget");
/// ```
///
/// `$factory` is any `fn() -> Arc<dyn RouteSelector>`.
#[macro_export]
macro_rules! register_route_selector {
    ($factory:expr, name = $name:expr $(,)?) => {
        ::inventory::submit! {
            $crate::RouteSelectorRegistration {
                name: $name,
                factory: || ::std::sync::Arc::new($factory()),
            }
        }
    };
}

/// The process-global set of registered [`RouteSelector`]s, resolved once.
pub struct RouteSelectorEngine {
    selectors: Vec<Arc<dyn RouteSelector>>,
}

impl std::fmt::Debug for RouteSelectorEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSelectorEngine")
            .field("selectors", &self.selectors.len())
            .finish()
    }
}

impl RouteSelectorEngine {
    #[must_use]
    pub fn global() -> &'static Self {
        static ENGINE: OnceLock<RouteSelectorEngine> = OnceLock::new();
        ENGINE.get_or_init(|| Self {
            selectors: inventory::iter::<RouteSelectorRegistration>()
                .map(|reg| (reg.factory)())
                .collect(),
        })
    }

    #[must_use]
    pub fn has_selectors(&self) -> bool {
        !self.selectors.is_empty()
    }

    /// Runs each registered selector in registration order; the first to return
    /// a replacement route wins. Returns the replacement and the selector's
    /// name (for the audit descriptor), or `None` when no selector re-routes.
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
