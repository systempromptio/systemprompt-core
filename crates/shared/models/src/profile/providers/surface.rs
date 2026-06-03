//! Client-facing API surface for an upstream AI provider.
//!
//! [`ApiSurface`] names the *vendor API family* a provider's models are
//! advertised under, independent of the [`WireProtocol`] the gateway speaks to
//! reach it. A provider can speak the Anthropic wire yet not be the Anthropic
//! vendor: `minimax` declares `wire: anthropic` (so the gateway reuses the
//! Anthropic codec) and `surface: backend` (so its `MiniMax-M2` model is never
//! advertised to any client API, only reached through an explicit gateway
//! route). Keeping these two facts orthogonal is what stops a backend provider
//! from masquerading as an Anthropic model in a client's catalog.

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

    /// `backend` parses here (it is a valid declared surface) but callers that
    /// resolve a *client* selection must reject it: a backend provider is never
    /// a front door.
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

    /// The single definition of the advertisement rule.
    ///
    /// Every host-facing catalog (`/v1/models`, `/v1/bridge/profile`, the admin
    /// profile page) derives its exclusion of backend providers from here
    /// rather than re-checking the variant per call site.
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
