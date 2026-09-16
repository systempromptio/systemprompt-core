//! Where a persisted AI request came from: the client that produced it and the
//! inbound wire protocol it spoke.
//!
//! Both halves are closed enums so `ai_requests.client_kind` and
//! `ai_requests.wire_protocol` are CHECK-constrained columns, never free text.
//! [`ClientKind::from_user_agent_and_body`] is the one classifier shared by the
//! bridge (native-session attribution) and the gateway (audit row), so the two
//! can never disagree about which harness sent a request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::feedback::EvaluatorClient;

/// The client that produced an AI request.
///
/// `Other` is a real HTTP caller the gateway does not recognise as a harness
/// (a raw SDK, curl, a chat UI). `Internal` is a request the server made for
/// itself with no HTTP ingress. `Unknown` exists only for rows written before
/// attribution was recorded and for an old binary inserting during a deploy
/// window; live code never constructs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientKind {
    ClaudeCode,
    ClaudeDesktop,
    Codex,
    OpenCode,
    Hermes,
    Other,
    Internal,
    Unknown,
}

/// The protocol a request arrived on. `Internal` pairs with
/// [`ClientKind::Internal`]; `Unknown` pairs with [`ClientKind::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InboundWireProtocol {
    #[serde(rename = "anthropic.messages")]
    AnthropicMessages,
    #[serde(rename = "openai.chat")]
    OpenAiChat,
    #[serde(rename = "openai.responses")]
    OpenAiResponses,
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Client and wire protocol of one request, carried together so a producer
/// cannot record one half without the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestOrigin {
    pub client: ClientKind,
    pub wire: InboundWireProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OriginParseError {
    #[error("unknown client kind: {0}")]
    ClientKind(String),
    #[error("unknown inbound wire protocol: {0}")]
    WireProtocol(String),
    #[error("client kind {0} is not an evaluator harness")]
    NotAHarness(&'static str),
}

impl ClientKind {
    pub const ALL: [Self; 8] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::OpenCode,
        Self::Hermes,
        Self::Other,
        Self::Internal,
        Self::Unknown,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Hermes => "hermes",
            Self::Other => "other",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }

    /// Human label for dashboards and CLI tables.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
            Self::Hermes => "Hermes",
            Self::Other => "API client",
            Self::Internal => "Internal",
            Self::Unknown => "Unknown",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OriginParseError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| OriginParseError::ClientKind(value.to_owned()))
    }

    /// Classify a caller from its `User-Agent` and request body.
    ///
    /// Total: never panics and never returns [`Self::Internal`] or
    /// [`Self::Unknown`]. Precedence follows the bridge's original sniffing
    /// order. The body is parsed only when no user agent matched, to find the
    /// Codex turn-metadata marker that Codex sends without a distinctive agent.
    #[must_use]
    pub fn from_user_agent_and_body(user_agent: Option<&str>, body: &[u8]) -> Self {
        let user_agent = user_agent.unwrap_or_default().to_ascii_lowercase();
        if user_agent.contains("hermes") {
            return Self::Hermes;
        }
        if user_agent.contains("opencode") {
            return Self::OpenCode;
        }
        if user_agent.contains("codex") {
            return Self::Codex;
        }
        if user_agent.contains("claude-desktop") {
            return Self::ClaudeDesktop;
        }
        if user_agent.contains("claude-cli") || user_agent.contains("claude-code") {
            return Self::ClaudeCode;
        }
        if body_has_codex_marker(body) {
            return Self::Codex;
        }
        Self::Other
    }
}

// JSON: protocol boundary — the inference body is any host's wire shape.
fn body_has_codex_marker(body: &[u8]) -> bool {
    if body.is_empty() {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(body).is_ok_and(|value| {
        value
            .pointer("/client_metadata/x-codex-turn-metadata")
            .is_some()
    })
}

impl InboundWireProtocol {
    pub const ALL: [Self; 5] = [
        Self::AnthropicMessages,
        Self::OpenAiChat,
        Self::OpenAiResponses,
        Self::Internal,
        Self::Unknown,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic.messages",
            Self::OpenAiChat => "openai.chat",
            Self::OpenAiResponses => "openai.responses",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OriginParseError> {
        Self::ALL
            .into_iter()
            .find(|wire| wire.as_str() == value)
            .ok_or_else(|| OriginParseError::WireProtocol(value.to_owned()))
    }
}

impl RequestOrigin {
    pub const INTERNAL: Self = Self {
        client: ClientKind::Internal,
        wire: InboundWireProtocol::Internal,
    };

    #[must_use]
    pub const fn gateway(client: ClientKind, wire: InboundWireProtocol) -> Self {
        Self { client, wire }
    }
}

impl From<EvaluatorClient> for ClientKind {
    fn from(client: EvaluatorClient) -> Self {
        match client {
            EvaluatorClient::ClaudeCode => Self::ClaudeCode,
            EvaluatorClient::ClaudeDesktop => Self::ClaudeDesktop,
            EvaluatorClient::Codex => Self::Codex,
            EvaluatorClient::OpenCode => Self::OpenCode,
            EvaluatorClient::Hermes => Self::Hermes,
        }
    }
}

impl TryFrom<ClientKind> for EvaluatorClient {
    type Error = OriginParseError;

    fn try_from(kind: ClientKind) -> Result<Self, Self::Error> {
        match kind {
            ClientKind::ClaudeCode => Ok(Self::ClaudeCode),
            ClientKind::ClaudeDesktop => Ok(Self::ClaudeDesktop),
            ClientKind::Codex => Ok(Self::Codex),
            ClientKind::OpenCode => Ok(Self::OpenCode),
            ClientKind::Hermes => Ok(Self::Hermes),
            ClientKind::Other | ClientKind::Internal | ClientKind::Unknown => {
                Err(OriginParseError::NotAHarness(kind.as_str()))
            },
        }
    }
}
