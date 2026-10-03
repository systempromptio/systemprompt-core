//! Client-facing API surface for an upstream AI provider.
//!
//! [`ApiSurface`] names the *vendor API family* a provider's models are
//! advertised under, independent of the wire protocol the gateway speaks to
//! reach it. A provider can speak the Anthropic wire yet not be the Anthropic
//! vendor: `minimax` declares `wire: anthropic` (so the gateway reuses the
//! Anthropic codec) and `surface: backend` (so its `MiniMax-M2` model is never
//! advertised to any client API, only reached through an explicit gateway
//! route). Keeping these two facts orthogonal is what stops a backend provider
//! from masquerading as an Anthropic model in a client's catalog.
//!
//! [`without_context_variant`] strips the client-side context-window marker
//! from a requested model id, so the bridge and the gateway resolve the same
//! catalog entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ApiSurface {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "backend")]
    Backend,
}

impl ApiSurface {
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Backend => "backend",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAi),
            "gemini" => Some(Self::Gemini),
            "backend" => Some(Self::Backend),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_advertised(self) -> bool {
        !matches!(self, Self::Backend)
    }
}

impl std::fmt::Display for ApiSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_tag())
    }
}

// Why: Claude Code budgets a gateway model at 200k unless the user picks its
// `[1m]` variant (`claude-sonnet-5[1m]`). The suffix is a client-side context
// marker, not a vendor id: it resolves to the base catalog entry and never
// reaches the upstream.
const CONTEXT_VARIANT_SUFFIX: &str = "[1m]";

#[must_use]
pub fn without_context_variant(requested: &str) -> &str {
    requested
        .strip_suffix(CONTEXT_VARIANT_SUFFIX)
        .unwrap_or(requested)
}
