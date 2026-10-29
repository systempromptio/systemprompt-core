//! Errors raised by auth setup, including why a pasted token was refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::PAT_PREFIX;
use crate::config::gateways::{self, credential};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    Prefix,
    Separator,
    TooShort,
}

impl std::fmt::Display for TokenRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prefix => write!(f, "token must start with `{PAT_PREFIX}`"),
            Self::Separator => {
                f.write_str("token must contain a `.` separator (sp-live-<prefix>.<secret>)")
            },
            Self::TooShort => f.write_str("token looks too short — did the copy get truncated?"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Token(TokenRejection),
    #[error("gateway_url is empty")]
    EmptyGateway,
    #[error("cannot resolve {0}")]
    Unresolvable(&'static str),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<toml_edit::TomlError>,
    },
    #[error("{path}: gateway_url must be a nonempty string")]
    GatewayNotString { path: PathBuf },
    #[error(transparent)]
    ConfigWrite(#[from] crate::config::ConfigWriteError),
    #[error(transparent)]
    ConfigRead(#[from] crate::config::ConfigReadError),
    #[error("token cache: {0}")]
    Cache(#[source] std::io::Error),
    #[error("credential binding: {0}")]
    Binding(#[source] std::io::Error),
    #[error(transparent)]
    PluginOAuth(#[from] crate::auth::plugin_oauth::PluginOAuthError),
    #[error("device link: {0}")]
    DeviceLink(#[source] crate::auth::providers::AuthError),
    #[error("gateway changed during session sign-in")]
    GatewayMoved,
    #[error("task join: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("gateway: {0}")]
    Gateway(#[from] crate::gateway::GatewayError),
    #[error("cancelled")]
    Cancelled,
    #[error("PAT store: {0}")]
    Credential(#[from] credential::CredentialError),
    #[error(transparent)]
    Profile(#[from] gateways::ProfileError),
}
