//! Wire-protocol family for an upstream AI provider.
//!
//! [`WireProtocol`] names the *request/response shape* a provider speaks, not
//! the vendor. It is the single key that both the gateway's outbound adapters
//! and the agent-flow provider clients resolve a wire codec from: `minimax`
//! speaks [`WireProtocol::Anthropic`]; `moonshot` and `qwen` speak
//! [`WireProtocol::OpenAiChat`]. Decoupling the protocol from the provider name
//! is what lets a new vendor reuse an existing codec by declaring its protocol.

use serde::{Deserialize, Serialize};

use crate::schema::ProviderCapabilities;

/// The wire-format family a provider's endpoint speaks.
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
    /// The stable string tag used to resolve a codec/adapter for this protocol.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAiChat => "openai-chat",
            Self::OpenAiResponses => "openai-responses",
            Self::Gemini => "gemini",
        }
    }

    /// The JSON-Schema constructs this protocol's tool/output schema parser
    /// accepts. The wire codecs feed this to [`crate::schema::SchemaSanitizer`]
    /// so every tool schema is reduced to a shape the upstream will accept.
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
