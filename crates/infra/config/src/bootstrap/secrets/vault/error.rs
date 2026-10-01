//! Failure modes of the Vault / `OpenBao` KV v2 secrets provider.
//!
//! No variant carries a response body, a token, or a secret value. The only
//! upstream text that reaches an operator is Vault's own `errors[]` array,
//! truncated to [`MAX_VAULT_ERROR_CHARS`], because that array is documented to
//! hold policy diagnostics rather than secret material.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::BoxedSource;

pub(super) const MAX_VAULT_ERROR_CHARS: usize = 200;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VaultError {
    #[error("vault address is not a permitted outbound URL: {0}")]
    Address(#[source] BoxedSource),

    #[error("vault TLS CA certificate at {path} could not be loaded: {source}")]
    CaCertificate {
        path: String,
        #[source]
        source: BoxedSource,
    },

    #[error("vault HTTP client could not be built: {0}")]
    ClientBuild(#[source] reqwest::Error),

    #[error(
        "vault credential is missing: {name}. Provide it in the process environment; never write \
         a token into profile YAML."
    )]
    MissingCredential { name: String },

    #[error("vault credential file {path} could not be read: {source}")]
    CredentialFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("vault credential file {path} could not be read: file is empty")]
    EmptyCredentialFile { path: String },

    #[error("vault {method} login was rejected with HTTP {status}{detail}")]
    Auth {
        method: &'static str,
        status: u16,
        detail: String,
    },

    #[error("vault denied access to {mount}/{path} (HTTP 403){detail}")]
    Forbidden {
        mount: String,
        path: String,
        detail: String,
    },

    #[error("vault has no secret at {mount}/{path}")]
    NotFound { mount: String, path: String },

    #[error("vault returned HTTP {status} for {mount}/{path}{detail}")]
    Http {
        status: u16,
        mount: String,
        path: String,
        detail: String,
    },

    #[error("vault response body could not be read: {0}")]
    Body(#[source] reqwest::Error),

    #[error("vault response was not the expected KV v2 shape: {context}: {source}")]
    Malformed {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "vault response was not the expected KV v2 shape: {location} has no field '{field}' for \
         override key '{key}'"
    )]
    MissingField {
        location: String,
        field: String,
        key: String,
    },

    #[error("vault was unreachable after {attempts} attempt(s): {message}")]
    Exhausted { attempts: u32, message: String },
}

#[must_use]
pub(super) fn truncate_detail(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let clipped: String = trimmed.chars().take(MAX_VAULT_ERROR_CHARS).collect();
    format!(": {clipped}")
}
