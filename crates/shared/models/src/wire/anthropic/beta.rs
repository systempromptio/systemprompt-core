//! `anthropic-beta`: one opt-in feature flag, the comma-separated header that
//! carries several, and the policy that decides which of them an upstream is
//! sent.
//!
//! A client forwards the betas it would send to Anthropic's own API. Another
//! host serves a subset and rejects the rest, so a request is re-rendered
//! through [`BetaPolicy`] before it leaves the gateway rather than relayed
//! verbatim.
//!
//! Some betas gate a top-level body field as well as the header
//! ([`BETA_GATED_FIELDS`]): `context-management-*` admits `context_management`.
//! An upstream that is sent the field without the flag refuses the whole
//! request ("`context_management`: Extra inputs are not permitted"), so a flag
//! the gateway drops takes its field with it ([`strip_fields_gated_by`]). A
//! field a client sends with no flag at all is left alone: that is the
//! client's own contract with the upstream, not a seam the gateway opened.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
// JSON: protocol boundary — the gated fields live in a dynamic wire body.
use serde_json::{Map, Value};

pub const ANTHROPIC_BETA_HEADER: &str = "anthropic-beta";

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
    pub fn refused_by(self, policy: &BetaPolicy) -> Self {
        Self(self.0.into_iter().filter(|b| !policy.admits(b)).collect())
    }

    pub fn extend(&mut self, other: Self) {
        for beta in other.0 {
            if !self.0.iter().any(|b| b == &beta) {
                self.0.push(beta);
            }
        }
    }

    #[must_use]
    pub fn contains(&self, flag: &str) -> bool {
        self.0.iter().any(|b| b.as_str() == flag)
    }

    #[must_use]
    pub fn opens(&self, gate: &BetaGatedField) -> bool {
        self.0
            .iter()
            .any(|b| b.as_str().starts_with(gate.beta_prefix))
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

/// A top-level Messages body field that exists only under an `anthropic-beta`
/// flag. `beta_prefix` is the flag without its date suffix, so every version
/// of the beta opens the same field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BetaGatedField {
    pub field: &'static str,
    pub beta_prefix: &'static str,
}

pub const BETA_GATED_FIELDS: &[BetaGatedField] = &[
    BetaGatedField {
        field: "context_management",
        beta_prefix: "context-management-",
    },
    BetaGatedField {
        field: "mcp_servers",
        beta_prefix: "mcp-client-",
    },
    BetaGatedField {
        field: "container",
        beta_prefix: "code-execution-",
    },
];

// JSON: Anthropic Messages request body; beta-gated fields are stripped by key.
pub fn strip_fields_gated_by(
    body: &mut Map<String, Value>,
    dropped: &BetaHeader,
) -> Vec<&'static str> {
    BETA_GATED_FIELDS
        .iter()
        .filter(|gate| dropped.opens(gate) && body.remove(gate.field).is_some())
        .map(|gate| gate.field)
        .collect()
}
