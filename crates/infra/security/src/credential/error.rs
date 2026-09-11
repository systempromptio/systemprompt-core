//! Everything that can go wrong between a stored secret and an auth header.
//!
//! The variants are typed so that callers can branch, but their `Display`
//! text is load-bearing too: the gateway relays it into a `PreAudit` dispatch
//! error and an operator reads it in a log line. So each message names the
//! thing an operator can change — the secret, the key inside it, the token
//! endpoint — and never the value of any of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

/// A credential could not be parsed, scoped, or exchanged for a header.
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("service-account key is malformed: {0}")]
    Malformed(String),

    #[error(
        "endpoint '{endpoint}' needs a {field} to fill `{placeholder}`, but the secret is not a \
         service-account key (no project_id or region to fill it from)"
    )]
    MissingScope {
        endpoint: String,
        field: &'static str,
        placeholder: &'static str,
    },

    #[error("service-account private_key is not a valid RSA PEM: {0}")]
    SigningKey(String),

    #[error("could not sign the assertion: {0}")]
    Sign(String),

    #[error("system clock is before the unix epoch: {0}")]
    Clock(String),

    #[error("could not build the token-exchange client: {0}")]
    Client(String),

    #[error("token endpoint {uri} unreachable: {reason}")]
    Unreachable { uri: String, reason: String },

    #[error("token endpoint returned {status}: {body}")]
    Rejected { status: String, body: String },

    #[error("token endpoint returned an unreadable body: {0}")]
    UnreadableBody(String),
}
