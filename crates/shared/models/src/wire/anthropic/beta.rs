//! `anthropic-beta`: one opt-in feature flag, the comma-separated header that
//! carries several, and the policy that decides which of them an upstream is
//! sent.
//!
//! A client forwards the betas it would send to Anthropic's own API. Another
//! host serves a subset and rejects the rest, so a request is re-rendered
//! through [`BetaPolicy`] before it leaves the gateway rather than relayed
//! verbatim.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const ANTHROPIC_BETA_HEADER: &str = "anthropic-beta";

/// One `anthropic-beta` flag, for example `interleaved-thinking-2025-05-14`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct AnthropicBeta(String);

impl AnthropicBeta {
    #[must_use]
    pub fn new(flag: impl Into<String>) -> Self {
        Self(flag.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnthropicBeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The betas one request carries, in the order the client sent them, each once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BetaHeader(Vec<AnthropicBeta>);

impl BetaHeader {
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let mut betas: Vec<AnthropicBeta> = Vec::new();
        for flag in value.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            if !betas.iter().any(|b| b.as_str() == flag) {
                betas.push(AnthropicBeta::new(flag));
            }
        }
        Self(betas)
    }

    #[must_use]
    pub fn admitted_by(self, policy: &BetaPolicy) -> Self {
        Self(self.0.into_iter().filter(|b| policy.admits(b)).collect())
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn betas(&self) -> &[AnthropicBeta] {
        &self.0
    }

    #[must_use]
    pub fn render(&self) -> Option<String> {
        (!self.0.is_empty()).then(|| {
            self.0
                .iter()
                .map(AnthropicBeta::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
    }
}

/// Which forwarded betas an upstream is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BetaPolicy {
    ForwardAll,
    Only(BTreeSet<AnthropicBeta>),
}

impl BetaPolicy {
    #[must_use]
    pub fn admits(&self, beta: &AnthropicBeta) -> bool {
        match self {
            Self::ForwardAll => true,
            Self::Only(accepted) => accepted.contains(beta),
        }
    }
}
