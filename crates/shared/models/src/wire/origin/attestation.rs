//! The evidence tier behind a `client_kind`, and the body markers that make
//! up the `native-marker` tier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::OriginParseError;

/// How strongly the recorded client is evidenced; see the module head for
/// the ladder. Ordered strongest-first so `Ord` matches precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientAttestation {
    HostToken,
    BridgeSecret,
    Declared,
    NativeMarker,
    UserAgent,
    None,
    Internal,
    Unknown,
}

/// A structural marker of one harness found in the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeMarker {
    ClaudeMetadataUserId,
    CodexTurnMetadata,
    OpencodeSessionJson,
}

impl ClientAttestation {
    pub const ALL: [Self; 8] = [
        Self::HostToken,
        Self::BridgeSecret,
        Self::Declared,
        Self::NativeMarker,
        Self::UserAgent,
        Self::None,
        Self::Internal,
        Self::Unknown,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostToken => "host-token",
            Self::BridgeSecret => "bridge-secret",
            Self::Declared => "declared",
            Self::NativeMarker => "native-marker",
            Self::UserAgent => "user-agent",
            Self::None => "none",
            Self::Internal => "internal",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HostToken => "Attested by bridge host token",
            Self::BridgeSecret => "Via bridge, host unverified",
            Self::Declared => "Declared by client",
            Self::NativeMarker => "Native marker only",
            Self::UserAgent => "User-Agent only, unverified",
            Self::None => "No client evidence",
            Self::Internal => "Internal",
            Self::Unknown => "Unknown",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OriginParseError> {
        Self::ALL
            .into_iter()
            .find(|tier| tier.as_str() == value)
            .ok_or_else(|| OriginParseError::Attestation(value.to_owned()))
    }

    #[must_use]
    pub const fn is_bridge_channel(self) -> bool {
        matches!(self, Self::HostToken | Self::BridgeSecret)
    }
}

impl NativeMarker {
    pub const ALL: [Self; 3] = [
        Self::ClaudeMetadataUserId,
        Self::CodexTurnMetadata,
        Self::OpencodeSessionJson,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeMetadataUserId => "claude-metadata-user-id",
            Self::CodexTurnMetadata => "codex-turn-metadata",
            Self::OpencodeSessionJson => "opencode-session-json",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OriginParseError> {
        Self::ALL
            .into_iter()
            .find(|marker| marker.as_str() == value)
            .ok_or_else(|| OriginParseError::NativeMarker(value.to_owned()))
    }

    #[must_use]
    pub const fn client(self) -> super::ClientKind {
        match self {
            // Why: the Claude metadata.user_id grammar is shared by Claude
            // Code and Claude Desktop; the tier records that it is a marker,
            // not a verified host, and the historical default stands.
            Self::ClaudeMetadataUserId => super::ClientKind::ClaudeCode,
            Self::CodexTurnMetadata => super::ClientKind::Codex,
            Self::OpencodeSessionJson => super::ClientKind::OpenCode,
        }
    }
}
