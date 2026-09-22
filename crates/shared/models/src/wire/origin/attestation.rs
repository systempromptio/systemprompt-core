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
///
/// Claude Code stamps `metadata.user_id` in one of two grammars —
/// `user_<hex>_account_<uuid>_session_<uuid>` before 2.1.25x and a JSON
/// object `{"account_uuid","device_id","session_id"}` from then on — and,
/// on a third-party gateway, opens the system prompt with
/// `x-anthropic-billing-header: cc_version=…; cc_entrypoint=<entry>;`. The
/// entrypoint is the only body signal that tells Claude Desktop (Cowork:
/// `claude-desktop-3p`, `local-agent`) from the CLI (`cli`), so it outranks
/// the metadata grammars. Neither grammar was ever sent by OpenCode; a
/// former `opencode-session-json` marker matched the JSON one and mislabelled
/// every modern Claude Code request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeMarker {
    ClaudeDesktopEntrypoint,
    ClaudeCliEntrypoint,
    ClaudeMetadataUserId,
    ClaudeMetadataJson,
    CodexTurnMetadata,
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
    pub const ALL: [Self; 5] = [
        Self::ClaudeDesktopEntrypoint,
        Self::ClaudeCliEntrypoint,
        Self::ClaudeMetadataUserId,
        Self::ClaudeMetadataJson,
        Self::CodexTurnMetadata,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeDesktopEntrypoint => "claude-desktop-entrypoint",
            Self::ClaudeCliEntrypoint => "claude-cli-entrypoint",
            Self::ClaudeMetadataUserId => "claude-metadata-user-id",
            Self::ClaudeMetadataJson => "claude-metadata-json",
            Self::CodexTurnMetadata => "codex-turn-metadata",
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
            Self::ClaudeDesktopEntrypoint => super::ClientKind::ClaudeDesktop,
            // Why: both metadata.user_id grammars are shared by Claude Code
            // and Claude Desktop; without an entrypoint the tier records a
            // marker, not a verified host, and the historical default stands.
            Self::ClaudeCliEntrypoint | Self::ClaudeMetadataUserId | Self::ClaudeMetadataJson => {
                super::ClientKind::ClaudeCode
            },
            Self::CodexTurnMetadata => super::ClientKind::Codex,
        }
    }
}
