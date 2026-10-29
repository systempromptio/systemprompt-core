//! The error every gateway-profile operation returns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::{GatewayProfileId, ProfileIdError, credential};

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
    #[error(
        "this process is pinned to {path} by {var}; gateway profiles are unavailable while it is set"
    )]
    Pinned { path: PathBuf, var: String },
    #[error(transparent)]
    Id(#[from] ProfileIdError),
    #[error("no gateway profile named `{0}`")]
    Unknown(GatewayProfileId),
    #[error("`{0}` is the active gateway; switch to another before removing it")]
    RemoveActive(GatewayProfileId),
    #[error("a gateway profile for {0} already exists as `{1}`")]
    Duplicate(String, GatewayProfileId),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    Malformed {
        path: PathBuf,
        #[source]
        source: Box<toml_edit::TomlError>,
    },
    #[error("{path}: `active` must name a gateway profile: {source}")]
    BadActive {
        path: PathBuf,
        #[source]
        source: ProfileIdError,
    },
    #[error("{path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    ConfigWrite(#[from] super::super::ConfigWriteError),
    #[error(transparent)]
    ConfigRead(#[from] super::super::ConfigReadError),
    #[error(transparent)]
    Credential(#[from] credential::CredentialError),
}

impl ProfileError {
    pub(super) fn io(action: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}
