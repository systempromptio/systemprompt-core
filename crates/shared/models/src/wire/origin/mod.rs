//! Where a persisted AI request came from: the client that produced it, the
//! inbound wire protocol it spoke, and how strongly the client is evidenced.
//!
//! All three halves are closed enums so `ai_requests.client_kind`,
//! `ai_requests.wire_protocol` and `ai_requests.client_attestation` are
//! CHECK-constrained columns, never free text. This module is the normative
//! specification of client attribution; the hosted gateway documentation
//! restates it for operators.
//!
//! # Evidence tiers
//!
//! [`classify`] answers "which client sent this request?" by walking a ladder
//! of evidence, strongest first. The winning tier sets `client_kind` and
//! `client_attestation`; everything the wire carried is kept in
//! [`ClientEvidence`] so a classification can be audited or corrected later.
//!
//! | tier | header / signal | trust |
//! |------|-----------------|-------|
//! | `host-token` | bridge principal with `x-systemprompt-client-attestation: host-token`; the bridge verified a per-host HMAC token on its loopback and stamped `x-systemprompt-client` itself | cryptographic on the device, channel-bound to the bridge |
//! | `bridge-secret` | bridge principal with `x-systemprompt-client-attestation: bridge-secret`; the caller presented the raw loopback secret, so the host is taken from the lower tiers | channel verified, host not |
//! | `declared` | `x-systemprompt-client: <kind>` from any principal (or passed through on the secret path) | client-asserted, closed vocabulary |
//! | `native-marker` | a structural marker of one harness in the body ([`NativeMarker`]): Claude Code's billing entrypoint or `metadata.user_id` grammars, Codex turn metadata | structural, unforged in practice |
//! | `user-agent` | the exact first product token of `User-Agent` ([`ua_product`]) | weakest tier kept |
//! | `none` | nothing matched; `client_kind` is `other` | honest fallback |
//!
//! `internal` and `unknown` are never produced from live gateway traffic:
//! `internal` pairs with server-side producers and `unknown` with rows written
//! before attribution existed.
//!
//! A conflict between tiers (a host token naming `opencode` under a
//! `claude-cli` User-Agent) is never rejected: the strongest tier wins and the
//! evidence row records the rest. Two inputs are rejected with `400` instead:
//! an `x-systemprompt-client` value outside the vocabulary, and an
//! `x-systemprompt-client-attestation` header from anything but the bridge.
//!
//! The one wire vocabulary is [`ClientKind::as_str`]. The bridge's own host
//! ids (`codex-cli`) are baked into HMAC labels and rendered host configs, so
//! they stay bridge-internal and map through
//! [`ClientKind::from_bridge_host_id`] / [`ClientKind::bridge_host_id`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attestation;
mod classify;
mod evidence;
mod harness;

pub use attestation::{ClientAttestation, NativeMarker};
pub use classify::{
    ClassificationInput, ClassificationRejection, Classified, StainlessHeaders, classify,
    native_marker, ua_product,
};
pub use evidence::ClientEvidence;

use serde::{Deserialize, Serialize};

/// The client that produced an AI request.
///
/// `Other` is a real HTTP caller the gateway could not name (a raw SDK, curl,
/// a chat UI); its evidence row still records what it presented. `Internal`
/// is a request the server made for itself with no HTTP ingress. `Unknown`
/// exists only for rows written before attribution was recorded and for an
/// old binary inserting during a deploy window; live code never constructs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientKind {
    ClaudeCode,
    ClaudeDesktop,
    Codex,
    #[serde(rename = "opencode")]
    OpenCode,
    Hermes,
    Pi,
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

/// Client, wire protocol and attestation of one request, carried together so
/// a producer cannot record one without the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestOrigin {
    pub client: ClientKind,
    pub wire: InboundWireProtocol,
    pub attestation: ClientAttestation,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OriginParseError {
    #[error("unknown client kind: {0}")]
    ClientKind(String),
    #[error("unknown inbound wire protocol: {0}")]
    WireProtocol(String),
    #[error("unknown client attestation: {0}")]
    Attestation(String),
    #[error("unknown native marker: {0}")]
    NativeMarker(String),
    #[error("client kind {0} is not an evaluator harness")]
    NotAHarness(&'static str),
}

impl ClientKind {
    pub const ALL: [Self; 9] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::OpenCode,
        Self::Hermes,
        Self::Pi,
        Self::Other,
        Self::Internal,
        Self::Unknown,
    ];

    // Why: the vocabulary a client may declare excludes the two server-only
    // values; `other` is declarable so a client can say "nothing you know".
    pub const DECLARABLE: [Self; 7] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::OpenCode,
        Self::Hermes,
        Self::Pi,
        Self::Other,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Hermes => "hermes",
            Self::Pi => "pi",
            Self::Other => "other",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
            Self::Hermes => "Hermes",
            Self::Pi => "Pi",
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

    #[must_use]
    pub fn from_bridge_host_id(host_id: &str) -> Option<Self> {
        match host_id {
            "claude-code" => Some(Self::ClaudeCode),
            "claude-desktop" => Some(Self::ClaudeDesktop),
            "codex-cli" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            "hermes" => Some(Self::Hermes),
            _ => None,
        }
    }

    // Why: only the exact, lower-cased first product token is consulted —
    // `contains` would let any client name a harness by mentioning it.
    #[must_use]
    pub fn from_ua_product(product: &str) -> Option<Self> {
        match product {
            "claude-cli" | "claude-code" => Some(Self::ClaudeCode),
            "claude-desktop" => Some(Self::ClaudeDesktop),
            "codex_cli_rs" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            "hermes-agent" => Some(Self::Hermes),
            _ => None,
        }
    }

    // Why: Claude Desktop runs the Claude Code runtime, so its requests wear
    // the `claude-cli` User-Agent and the CLI's metadata grammar; a signal
    // naming either is agreement, not a conflict.
    #[must_use]
    pub const fn same_runtime(self, other: Self) -> bool {
        matches!(
            (self, other),
            (
                Self::ClaudeCode | Self::ClaudeDesktop,
                Self::ClaudeCode | Self::ClaudeDesktop
            )
        ) || (self as u8) == (other as u8)
    }

    #[must_use]
    pub const fn bridge_host_id(self) -> Option<&'static str> {
        match self {
            Self::ClaudeCode => Some("claude-code"),
            Self::ClaudeDesktop => Some("claude-desktop"),
            Self::Codex => Some("codex-cli"),
            Self::OpenCode => Some("opencode"),
            Self::Hermes => Some("hermes"),
            Self::Pi | Self::Other | Self::Internal | Self::Unknown => None,
        }
    }
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
        attestation: ClientAttestation::Internal,
    };

    #[must_use]
    pub const fn gateway(
        client: ClientKind,
        wire: InboundWireProtocol,
        attestation: ClientAttestation,
    ) -> Self {
        Self {
            client,
            wire,
            attestation,
        }
    }
}
