//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::{PluginId, Sha256Digest, SkillId};

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("hash mismatch for {what}: expected {expected}, got {actual}")]
    HashMismatch {
        what: String,
        expected: Sha256Digest,
        actual: String,
    },
    #[error("unsafe path in manifest: {0}")]
    UnsafePath(String),
    #[error("unsafe plugin id in manifest: {0}")]
    UnsafePluginId(PluginId),
    #[error("unsafe skill id in manifest: {0}")]
    UnsafeSkillId(SkillId),
    #[error("unsafe agent name in manifest: {0}")]
    UnsafeAgentName(String),
    #[error(
        "manifest contains a plugin with reserved id `{0}` (used by bridge for managed \
         skills/agents/mcp)"
    )]
    ReservedPluginId(PluginId),
    #[error("plugin fetch failed: {0}")]
    PluginFetch(#[from] crate::gateway::GatewayError),
    #[error("io error in {context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    #[error("serialize {what}: {source}")]
    Serialize {
        what: String,
        source: serde_json::Error,
    },
    #[error("plugin hook token: {0}")]
    PluginOAuth(#[from] crate::auth::plugin_oauth::PluginOAuthError),
    #[error("toml {what}: {source}")]
    Toml {
        what: String,
        #[source]
        source: TomlError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TomlError {
    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),
    #[error(transparent)]
    Deserialize(#[from] toml::de::Error),
}
