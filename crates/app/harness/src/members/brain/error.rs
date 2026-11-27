//! How the instance's brain answers map to failures.
//!
//! On top of the client's own mapping (`401`, `429`, `5xx`, an undecodable
//! body, an answer over its size limit, all carried as
//! [`ClientError`](systemprompt_device::client::ClientError)): a `404` with
//! `error_key` `brain_disabled`, or with none (the routes are not mounted), is
//! [`BrainError::Disabled`]; any other `404` is [`BrainError::NotFound`]; `403`
//! is [`BrainError::Forbidden`]; `409` `fact_tombstoned` is
//! [`BrainError::Tombstoned`] and any other `409` [`BrainError::Conflict`];
//! `400` and `422` are [`BrainError::Invalid`]. The instance's own message is
//! never kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use systemprompt_device::client::{ClientError, TransportFailure};
use systemprompt_models::bridge::envelope::ErrorEnvelope;

const ERROR_KEY_MAX: usize = 64;
const SESSION_MISMATCH_KEY: &str = "session_mismatch";
const DISABLED_KEY: &str = "brain_disabled";
const TOMBSTONED_KEY: &str = "fact_tombstoned";

/// Every way a brain call can fail.
#[derive(Debug, thiserror::Error)]
pub enum BrainError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("the brain is not enabled on this instance")]
    Disabled,
    #[error("no instance that admits the brain extension is applied")]
    NoInstance,
    #[error("the instance has not yet confirmed that its brain is enabled")]
    Unconfirmed,
    #[error("{endpoint}: no such fact or proposal is visible to this account")]
    NotFound {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the instance refused the action for this account")]
    Forbidden {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the fact was erased and may not be recorded again")]
    Tombstoned { endpoint: &'static str },
    #[error("{endpoint}: the request conflicts with the stored state")]
    Conflict {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the instance refused the input")]
    Invalid {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("refused before sending: {what}")]
    InvalidInput { what: &'static str },
}

impl BrainError {
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        match self {
            Self::Client(error) => error.is_transient(),
            _ => false,
        }
    }

    #[must_use]
    pub fn error_key(&self) -> Option<&str> {
        match self {
            Self::NotFound { error_key, .. }
            | Self::Forbidden { error_key, .. }
            | Self::Conflict { error_key, .. }
            | Self::Invalid { error_key, .. }
            | Self::Client(ClientError::Unauthorized { error_key, .. }) => error_key.as_deref(),
            Self::Disabled => Some(DISABLED_KEY),
            Self::Tombstoned { .. } => Some(TOMBSTONED_KEY),
            _ => None,
        }
    }
}

pub(crate) fn classify(
    endpoint: &'static str,
    status: u16,
    headers: &HeaderMap,
    body: &[u8],
) -> BrainError {
    // Why: discard-ok: a non-envelope body still classifies by status.
    let envelope: ErrorEnvelope = serde_json::from_slice(body).unwrap_or_default();
    let error_key = plain_key(envelope.error_key.as_deref());
    match status {
        401 => BrainError::Client(refusal(endpoint, error_key)),
        429 => BrainError::Client(ClientError::RateLimited {
            endpoint,
            retry_after: retry_after(headers, Utc::now()),
        }),
        500.. => BrainError::Client(ClientError::Unreachable {
            endpoint,
            cause: TransportFailure::Status { status },
        }),
        404 => match error_key {
            None => BrainError::Disabled,
            Some(key) if key == DISABLED_KEY => BrainError::Disabled,
            Some(key) => BrainError::NotFound {
                endpoint,
                error_key: Some(key),
            },
        },
        403 => BrainError::Forbidden {
            endpoint,
            error_key,
        },
        409 if error_key.as_deref() == Some(TOMBSTONED_KEY) => BrainError::Tombstoned { endpoint },
        409 => BrainError::Conflict {
            endpoint,
            error_key,
        },
        400 | 422 => BrainError::Invalid {
            endpoint,
            error_key,
        },
        300..=399 => BrainError::Client(protocol(
            endpoint,
            status,
            "the instance answered with a redirect, which is never followed".to_owned(),
        )),
        _ => BrainError::Client(protocol(
            endpoint,
            status,
            format!("unexpected status {status}"),
        )),
    }
}

pub(crate) fn refusal(endpoint: &'static str, error_key: Option<String>) -> ClientError {
    if error_key.as_deref() == Some(SESSION_MISMATCH_KEY) {
        ClientError::SessionMismatch { endpoint }
    } else {
        ClientError::Unauthorized {
            endpoint,
            error_key,
        }
    }
}

pub(crate) fn decode_failure(
    endpoint: &'static str,
    status: u16,
    error: &serde_json::Error,
) -> ClientError {
    protocol(
        endpoint,
        status,
        format!(
            "the answer does not decode ({:?} error at line {}, column {})",
            error.classify(),
            error.line(),
            error.column()
        ),
    )
}

pub(crate) const fn protocol(endpoint: &'static str, status: u16, what: String) -> ClientError {
    ClientError::Protocol {
        endpoint,
        status: Some(status),
        what,
        detail: None,
    }
}

pub(crate) fn plain_key(raw: Option<&str>) -> Option<String> {
    let key = raw?.trim();
    let plain = !key.is_empty()
        && key.len() <= ERROR_KEY_MAX
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b));
    plain.then(|| key.to_owned())
}

fn retry_after(headers: &HeaderMap, now: DateTime<Utc>) -> Option<Duration> {
    let raw = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
        return raw.parse::<u64>().ok().map(Duration::from_secs);
    }
    let at = DateTime::parse_from_rfc2822(raw).ok()?.with_timezone(&Utc);
    Some((at - now).to_std().unwrap_or(Duration::ZERO))
}
