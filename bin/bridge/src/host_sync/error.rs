//! Sync-apply error types.
//!
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
    #[error("skills {first} and {second} both map to the folder {dir}; rename one")]
    SkillDirCollision {
        dir: String,
        first: String,
        second: String,
    },
    #[error("unsafe agent name in manifest: {0}")]
    UnsafeAgentName(String),
    #[error("plugin fetch failed: {0}")]
    PluginFetch(#[from] crate::gateway::GatewayError),
    #[error("gateway changed to {current} while syncing {started_for}")]
    Superseded {
        started_for: String,
        current: String,
    },
    #[error("{what} needs administrator approval: {detail}")]
    ElevationRequired { what: &'static str, detail: String },
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
    #[error(transparent)]
    ForeignShape(#[from] ForeignShape),
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
    #[error(transparent)]
    Edit(#[from] Box<toml_edit::TomlError>),
}

/// A key the bridge owns already holds a value of a shape the bridge does not
/// write (a scalar where a table is expected, a list where a mapping is). The
/// file is the user's; it is never rewritten to fit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path}: `{key}` is {found}, expected {expected}; not rewriting a foreign value")]
pub struct ForeignShape {
    pub path: String,
    pub key: String,
    pub found: &'static str,
    pub expected: &'static str,
}

impl From<ForeignShape> for std::io::Error {
    fn from(e: ForeignShape) -> Self {
        Self::new(std::io::ErrorKind::InvalidData, e)
    }
}
