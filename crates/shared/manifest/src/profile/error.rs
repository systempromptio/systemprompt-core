//! Profile loading and validation error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use thiserror::Error;

pub type ProfileResult<T> = Result<T, ProfileError>;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("Failed to read profile {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse profile {path}: {source}")]
    ParseYaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Failed to serialize profile: {0}")]
    SerializeYaml(#[source] serde_yaml::Error),

    #[error("Invalid profile path: {path}")]
    InvalidProfilePath { path: PathBuf },

    #[error("Failed to write profile {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "Profile overlay {path} does not exist. A profile is profiles/<env>/profile.yaml, \
         optionally layered over profiles/base.yaml"
    )]
    MissingOverlay { path: PathBuf },

    #[error(
        "Profile {path} uses the pre-0.73 shape (`{key}`: {reason}). Run `{}` to rewrite it.",
        super::legacy::MIGRATE_COMMAND
    )]
    LegacyShape {
        path: PathBuf,
        key: String,
        reason: String,
    },

    #[error(
        "Profile {path} states `{field}`, which is derived from `{source_field}` at load. \
         Delete it (run `{}` to rewrite the profile).",
        super::legacy::MIGRATE_COMMAND
    )]
    DerivedField {
        path: PathBuf,
        field: String,
        source_field: String,
    },

    #[error("server.external_url '{url}' is invalid")]
    UnparsableExternalUrl {
        url: String,
        #[source]
        source: url::ParseError,
    },

    #[error("server.external_url '{url}' must be an absolute http:// or https:// URL with a host")]
    ExternalUrlNotHttp { url: String },

    #[error("Profile '{name}' validation failed:\n  - {}", errors.join("\n  - "))]
    Validation { name: String, errors: Vec<String> },

    #[error(
        "secrets.source '{secrets_source}' requires secrets.secrets_path to point at the secrets.json \
         location"
    )]
    SecretsPathRequired { secrets_source: &'static str },

    #[error("secrets.source 'vault' requires a secrets.vault block")]
    VaultBlockRequired,

    #[error("secrets.vault is only valid with secrets.source 'vault', got '{secrets_source}'")]
    VaultBlockUnexpected { secrets_source: &'static str },

    #[error("secrets.source 'external' requires a secrets.external block")]
    ExternalBlockRequired,

    #[error(
        "secrets.external is only valid with secrets.source 'external', got '{secrets_source}'"
    )]
    ExternalBlockUnexpected { secrets_source: &'static str },

    #[error("{field} must be at least 60, got {secs}")]
    RefreshIntervalOutOfRange { field: &'static str, secs: u64 },

    #[error(
        "secrets.refresh_secs is only valid with secrets.source 'file', got '{secrets_source}'"
    )]
    RefreshSecsUnexpected { secrets_source: &'static str },
}
