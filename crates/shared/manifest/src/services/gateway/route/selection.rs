//! Deployment selection across a route's chain, and the chain's
//! context-window fallbacks.
//!
//! [`SelectionStrategy`] decides which deployment of a chain a request tries
//! first. `ordered` (the default) is the chain order. `weighted` draws the
//! first attempt by each deployment's `weight` (default 1) among the healthy
//! deployments and tries the rest in descending weight. `least_busy` sends the
//! first attempt to the healthy deployment with the fewest requests in flight
//! in this process. Failover after the first attempt is unchanged. A `weight`
//! under `ordered` is accepted and ignored, so switching strategy is a
//! one-line change.
//!
//! `context_fallbacks` are deployments outside the failover chain: a request
//! whose estimated input does not fit the selected deployment's context window
//! is dispatched to the first of them whose model's window fits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::{ChainSelection, GatewayRoute};

/// How the first attempt is chosen among a chain's deployments.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStrategy {
    #[default]
    Ordered,
    Weighted,
    LeastBusy,
}

impl SelectionStrategy {
    #[must_use]
    pub const fn is_ordered(&self) -> bool {
        matches!(self, Self::Ordered)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ordered => "ordered",
            Self::Weighted => "weighted",
            Self::LeastBusy => "least_busy",
        }
    }
}

impl std::fmt::Display for SelectionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl GatewayRoute {
    #[must_use]
    pub fn effective_weight(&self) -> u32 {
        self.weight.unwrap_or(1)
    }

    #[must_use]
    pub fn context_fallback_views(&self, selection: &ChainSelection<'_>) -> Vec<Self> {
        let (deployments, strategy) = match selection {
            ChainSelection::Unmapped { .. } => return Vec::new(),
            ChainSelection::Route => (&self.context_fallbacks, self.strategy),
            ChainSelection::Scope { chain, .. } => (&chain.context_fallbacks, chain.strategy),
        };
        deployments
            .iter()
            .map(|d| self.view_of(d, strategy))
            .collect()
    }

    #[must_use]
    pub fn all_context_fallbacks(&self) -> Vec<(Option<&str>, Vec<Self>)> {
        let mut out = vec![(None, self.context_fallback_views(&ChainSelection::Route))];
        if let Some(scoped) = self.by_scope.as_ref() {
            for (value, chain) in &scoped.chains {
                let selection = ChainSelection::Scope {
                    dimension: &scoped.dimension,
                    value: value.as_str(),
                    chain,
                };
                out.push((
                    Some(value.as_str()),
                    self.context_fallback_views(&selection),
                ));
            }
        }
        out
    }
}
