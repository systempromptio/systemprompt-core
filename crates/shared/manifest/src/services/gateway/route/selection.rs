//! Deployment selection across a resolved chain.
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
//! A route's `context_fallbacks` are models outside the failover chain: a
//! request whose estimated input does not fit the selected model's context
//! window is dispatched to the first of them whose window fits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

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
