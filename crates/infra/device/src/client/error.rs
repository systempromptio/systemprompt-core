//! The client's one failure type.
//!
//! Errors name the route by method and path template (`POST
//! /v1/auth/bridge/pat`), never a full URL or a query. An instance's own words
//! are kept only as a bounded excerpt with anything shaped like a credential
//! redacted, and only in a field, never in `Display`. Decode failures report
//! the category and position, never the offending value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sp_protocol::OriginError;
use sp_protocol::view::{KeyFingerprint, SignInFailure, SyncErrorKind};

use crate::credentials::CredentialError;

const EXCERPT_CHARS: usize = 160;

const TOKEN_RUN_MIN: usize = 24;

const ERROR_KEY_MAX: usize = 64;

/// Every way a call to an instance can fail.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("{endpoint}: the instance refused the credential")]
    Unauthorized {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the instance refused the session this token names")]
    SessionMismatch { endpoint: &'static str },
    #[error("{endpoint}: the instance is rate limiting this client")]
    RateLimited {
        endpoint: &'static str,
        retry_after: Option<Duration>,
    },
    #[error("{endpoint}: the instance could not be reached")]
    Unreachable {
        endpoint: &'static str,
        #[source]
        cause: TransportFailure,
    },
    #[error("{endpoint}: the manifest signature does not verify")]
    SignatureInvalid { endpoint: &'static str },
    #[error("the instance now signs its manifest with key {offered}, not the pinned {pinned}")]
    KeyChanged {
        pinned: KeyFingerprint,
        offered: KeyFingerprint,
    },
    #[error("{endpoint}: {what}")]
    Incompatible {
        endpoint: &'static str,
        what: Incompatibility,
    },
    #[error("{endpoint}: {what}")]
    Protocol {
        endpoint: &'static str,
        status: Option<u16>,
        what: String,
        detail: Option<String>,
    },
    #[error("refused before sending: {what}")]
    InvalidInput { what: &'static str },
    #[error("the instance URL is refused")]
    Origin(#[source] OriginError),
    #[error("no session token is available")]
    Credential(#[source] CredentialError),
}

/// Why an address is not an instance this client can talk to.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Incompatibility {
    #[error("the address is not a systemprompt instance")]
    NotAnInstance,
    #[error(
        "the instance publishes discovery contract {found:?}, which this client does not speak"
    )]
    DiscoveryContract { found: String },
    #[error("the manifest needs schema {found}, newer than this client supports")]
    ManifestSchema { found: u32 },
}

/// What kept a request from getting an answer: the transport, a timeout (a
/// transport error whose `is_timeout` is set) or a 5xx.
#[derive(Debug, thiserror::Error)]
#[expect(
    variant_size_differences,
    reason = "reqwest::Error is already a single boxed pointer"
)]
pub enum TransportFailure {
    #[error("the request did not complete")]
    Request(#[source] reqwest::Error),
    #[error("the instance answered status {status}")]
    Status { status: u16 },
}

impl TransportFailure {
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Request(error) if error.is_timeout())
    }
}

impl ClientError {
    #[must_use]
    pub fn sign_in_failure(&self) -> SignInFailure {
        match self {
            Self::Unauthorized { .. } | Self::SessionMismatch { .. } => SignInFailure::Refused,
            Self::RateLimited { .. } => SignInFailure::RateLimited,
            Self::Unreachable { .. } => SignInFailure::Unreachable,
            Self::Incompatible { .. } => SignInFailure::Incompatible,
            Self::SignatureInvalid { .. }
            | Self::KeyChanged { .. }
            | Self::Protocol { .. }
            | Self::InvalidInput { .. }
            | Self::Origin(_) => SignInFailure::Failed,
            Self::Credential(error) => match error {
                CredentialError::SignedOut | CredentialError::Refused => SignInFailure::Refused,
                CredentialError::Insecure => SignInFailure::Insecure,
                CredentialError::Mint(inner) => inner.sign_in_failure(),
            },
        }
    }

    #[must_use]
    pub fn sync_error_kind(&self) -> SyncErrorKind {
        match self {
            Self::Unauthorized { .. } | Self::SessionMismatch { .. } => SyncErrorKind::Refused,
            Self::RateLimited { .. } => SyncErrorKind::RateLimited,
            Self::Unreachable { .. } => SyncErrorKind::Unreachable,
            Self::KeyChanged { .. } => SyncErrorKind::KeyChanged,
            Self::SignatureInvalid { .. }
            | Self::Incompatible { .. }
            | Self::Protocol { .. }
            | Self::InvalidInput { .. }
            | Self::Origin(_) => SyncErrorKind::Invalid,
            Self::Credential(error) => match error {
                CredentialError::SignedOut | CredentialError::Refused => SyncErrorKind::Refused,
                CredentialError::Insecure => SyncErrorKind::Insecure,
                CredentialError::Mint(inner) => inner.sync_error_kind(),
            },
        }
    }

    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Unauthorized { .. } | Self::SessionMismatch { .. } => Some(401),
            Self::RateLimited { .. } => Some(429),
            Self::Unreachable {
                cause: TransportFailure::Status { status },
                ..
            } => Some(*status),
            Self::Protocol { status, .. } => *status,
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized { .. })
    }

    #[must_use]
    pub const fn refuses_token(&self) -> bool {
        matches!(
            self,
            Self::Unauthorized { .. } | Self::SessionMismatch { .. }
        )
    }

    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Unreachable { .. } | Self::RateLimited { .. })
    }
}

pub(crate) fn excerpt(raw: &str) -> Option<String> {
    let mut out = String::new();
    for word in raw.split_whitespace() {
        if out.chars().count() >= EXCERPT_CHARS {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(if looks_secret(word) {
            "<redacted>"
        } else {
            word
        });
    }
    let bounded: String = out
        .chars()
        .filter(|c| !c.is_control())
        .take(EXCERPT_CHARS)
        .collect();
    (!bounded.is_empty()).then_some(bounded)
}

pub(crate) fn error_key(raw: Option<&str>) -> Option<String> {
    let key = raw?.trim();
    let plain = !key.is_empty()
        && key.len() <= ERROR_KEY_MAX
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b));
    plain.then(|| key.to_owned())
}

fn looks_secret(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if lower.contains("sp-live-")
        || lower.contains("sp_device_")
        || lower.contains("eyj")
        || lower.contains('?')
    {
        return true;
    }
    let mut run = 0usize;
    for b in word.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.+/=~".contains(&b) {
            run += 1;
            if run >= TOKEN_RUN_MIN {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}
