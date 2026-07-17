//! Wire-protocol family for an upstream AI provider.
//!
//! [`WireProtocol`] names the *request/response shape* a provider speaks, not
//! the vendor. It is the single key that both the gateway's outbound adapters
//! and the agent-flow provider clients resolve a wire codec from: `minimax`
//! speaks [`WireProtocol::Anthropic`]; `moonshot` and `qwen` speak
//! [`WireProtocol::OpenAiChat`]. Decoupling the protocol from the provider name
//! is what lets a new vendor reuse an existing codec by declaring its protocol.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::surface::ApiSurface;
use crate::schema::ProviderCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum WireProtocol {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai-chat", alias = "openai_chat", alias = "openai")]
    OpenAiChat,
    #[serde(rename = "openai-responses", alias = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "gemini")]
    Gemini,
}

impl WireProtocol {
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAiChat => "openai-chat",
            Self::OpenAiResponses => "openai-responses",
            Self::Gemini => "gemini",
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "anthropic" => Some(Self::Anthropic),
            "openai-chat" | "openai_chat" | "openai" => Some(Self::OpenAiChat),
            "openai-responses" | "openai_responses" => Some(Self::OpenAiResponses),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }

    /// The *implied* default surface only — the authoritative one is the
    /// explicit [`super::ProviderEntry::surface`] field, which can advertise a
    /// provider under a different family or none at all (`backend`).
    #[must_use]
    pub const fn surface(self) -> ApiSurface {
        match self {
            Self::Anthropic => ApiSurface::Anthropic,
            Self::OpenAiChat | Self::OpenAiResponses => ApiSurface::OpenAi,
            Self::Gemini => ApiSurface::Gemini,
        }
    }

    #[must_use]
    pub const fn schema_capabilities(self) -> ProviderCapabilities {
        match self {
            Self::Anthropic => ProviderCapabilities::anthropic(),
            Self::OpenAiChat | Self::OpenAiResponses => ProviderCapabilities::openai(),
            Self::Gemini => ProviderCapabilities::gemini(),
        }
    }
}

impl std::fmt::Display for WireProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_tag())
    }
}
