//! How the instance's Drive answers map to failures.
//!
//! On top of the client's own mapping (`401`, `429`, `5xx`, an undecodable
//! body, an answer over its size limit, all carried as [`ClientError`]): a
//! `404` is [`DriveError::NotFound`], also when the instance has no document
//! store at all; a `403` whose `error_key` starts `agent_` is
//! [`DriveError::AgentRefused`] with the capability that was missing; any other
//! `403` is [`DriveError::Forbidden`]; a `409` is [`DriveError::Conflict`] with
//! the head the instance reports, at the top level of the body or under
//! `detail`. The instance's own message is never kept.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use systemprompt_device::client::{ClientError, TransportFailure};
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::documents::client::DriveErrorBody;

const ERROR_KEY_MAX: usize = 64;
const SESSION_MISMATCH_KEY: &str = "session_mismatch";
const SUGGEST_ONLY_KEY: &str = "agent_suggest_only";
const FORBIDDEN_KEY: &str = "agent_forbidden";

/// What an agent refusal says was missing: `edit` (the cap is `suggest`: write
/// with `mode: propose`) or a person (sharing, replacing content: only the user
/// can, in the instance's own UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeededCapability {
    Edit,
    Person,
    Other,
}

impl fmt::Display for NeededCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Edit => "edit access",
            Self::Person => "the person themself",
            Self::Other => "a capability the agent lacks",
        })
    }
}

/// The head a conflicting write lost to, as far as the instance said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConflictHead {
    pub head_revision_id: Option<DocumentRevisionId>,
    pub head_content_sha256: Option<String>,
    pub base_revision_id: Option<DocumentRevisionId>,
}

/// Every way a Drive call can fail.
#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("{endpoint}: no such document or folder is visible to this account")]
    NotFound {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the instance does not let the agent do this; it needs {capability}")]
    AgentRefused {
        endpoint: &'static str,
        error_key: String,
        capability: NeededCapability,
    },
    #[error("{endpoint}: the instance refused the request")]
    Forbidden {
        endpoint: &'static str,
        error_key: Option<String>,
    },
    #[error("{endpoint}: the request conflicts with the document as it is now")]
    Conflict {
        endpoint: &'static str,
        error_key: Option<String>,
        head: ConflictHead,
    },
    #[error("{endpoint}: part {part} reached the instance altered")]
    PartAltered { endpoint: &'static str, part: u32 },
    #[error("the transfer was cancelled")]
    Cancelled,
    #[error("refused before sending: {what}")]
    InvalidInput { what: &'static str },
}

impl DriveError {
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        match self {
            Self::Client(error) => error.is_transient(),
            _ => false,
        }
    }

    #[must_use]
    pub const fn conflict_head(&self) -> Option<&ConflictHead> {
        match self {
            Self::Conflict { head, .. } => Some(head),
            _ => None,
        }
    }

    #[must_use]
    pub fn error_key(&self) -> Option<&str> {
        match self {
            Self::NotFound { error_key, .. }
            | Self::Forbidden { error_key, .. }
            | Self::Conflict { error_key, .. }
            | Self::Client(ClientError::Unauthorized { error_key, .. }) => error_key.as_deref(),
            Self::AgentRefused { error_key, .. } => Some(error_key),
            _ => None,
        }
    }
}

pub(crate) fn classify(
    endpoint: &'static str,
    status: u16,
    headers: &HeaderMap,
    body: &[u8],
) -> DriveError {
    // Why: discard-ok: a body that is not error JSON still classifies by status.
    let parsed: DriveErrorBody = serde_json::from_slice(body).unwrap_or_default();
    let error_key = plain_key(parsed.error_key.as_deref());
    match status {
        401 if error_key.as_deref() == Some(SESSION_MISMATCH_KEY) => {
            DriveError::Client(ClientError::SessionMismatch { endpoint })
        },
        401 => DriveError::Client(ClientError::Unauthorized {
            endpoint,
            error_key,
        }),
        429 => DriveError::Client(ClientError::RateLimited {
            endpoint,
            retry_after: retry_after(headers, Utc::now()),
        }),
        500.. => DriveError::Client(ClientError::Unreachable {
            endpoint,
            cause: TransportFailure::Status { status },
        }),
        404 => DriveError::NotFound {
            endpoint,
            error_key,
        },
        403 => match error_key {
            Some(key) if key.starts_with("agent_") => {
                let capability = match key.as_str() {
                    SUGGEST_ONLY_KEY => NeededCapability::Edit,
                    FORBIDDEN_KEY => NeededCapability::Person,
                    _ => NeededCapability::Other,
                };
                DriveError::AgentRefused {
                    endpoint,
                    error_key: key,
                    capability,
                }
            },
            other => DriveError::Forbidden {
                endpoint,
                error_key: other,
            },
        },
        409 => DriveError::Conflict {
            endpoint,
            error_key,
            head: conflict_head(&parsed),
        },
        300..=399 => DriveError::Client(ClientError::Protocol {
            endpoint,
            status: Some(status),
            what: "the instance answered with a redirect, which is never followed".to_owned(),
            detail: None,
        }),
        _ => DriveError::Client(ClientError::Protocol {
            endpoint,
            status: Some(status),
            what: format!("unexpected status {status}"),
            detail: None,
        }),
    }
}

pub(crate) fn decode_failure(
    endpoint: &'static str,
    status: u16,
    error: &serde_json::Error,
) -> ClientError {
    ClientError::Protocol {
        endpoint,
        status: Some(status),
        what: format!(
            "the answer does not decode ({:?} error at line {}, column {})",
            error.classify(),
            error.line(),
            error.column()
        ),
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

pub(crate) fn revision_id(raw: Option<&str>) -> Option<DocumentRevisionId> {
    let raw = raw?.trim();
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).ok()
}

pub(crate) fn retry_after(headers: &HeaderMap, now: DateTime<Utc>) -> Option<Duration> {
    let raw = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
        return raw.parse::<u64>().ok().map(Duration::from_secs);
    }
    let at = DateTime::parse_from_rfc2822(raw).ok()?.with_timezone(&Utc);
    Some((at - now).to_std().unwrap_or(Duration::ZERO))
}

fn conflict_head(body: &DriveErrorBody) -> ConflictHead {
    let detail = body.detail.as_ref();
    let content = body
        .head_content_sha256
        .as_deref()
        .or_else(|| detail.and_then(|d| d.head_content_sha256.as_deref()))
        .filter(|sha| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase);
    let base = body
        .base_revision_id
        .as_deref()
        .or_else(|| detail.and_then(|d| d.base_revision_id.as_deref()));
    ConflictHead {
        head_revision_id: revision_id(body.head_revision_id()),
        head_content_sha256: content,
        base_revision_id: revision_id(base),
    }
}
