//! Validating a filing: bounded title, body, note, fingerprint key and
//! client facts, and the fingerprint that folds a report's repeats.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sha2::{Digest, Sha256};
use systemprompt_identifiers::UserId;
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity};

use super::error::{IssuesError, IssuesResult};
use super::repository::NewIssue;

const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_CHARS: usize = 20_000;
pub(super) const MAX_NOTE_CHARS: usize = 4_000;
const MAX_FINGERPRINT_KEY_CHARS: usize = 512;
const MAX_CLIENT_BYTES: usize = 8_192;

/// A report as the reporter or the filing tool states it.
#[derive(Debug, Clone)]
pub struct IssueFiling {
    pub reporter: UserId,
    pub via_agent: Option<String>,
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    pub title: String,
    pub body: String,
    pub fingerprint_key: Option<String>,
    pub context: IssueContext,
    pub client: serde_json::Value,
}

pub(super) fn bounded(field: &'static str, value: &str, max: usize) -> IssuesResult<()> {
    if value.chars().count() > max {
        return Err(IssuesError::Invalid {
            field,
            reason: format!("at most {max} characters"),
        });
    }
    Ok(())
}

pub(super) fn validated(filing: IssueFiling) -> IssuesResult<NewIssue> {
    let title = filing.title.trim().to_owned();
    if title.is_empty() {
        return Err(IssuesError::Invalid {
            field: "title",
            reason: "must not be empty".to_owned(),
        });
    }
    bounded("title", &title, MAX_TITLE_CHARS)?;
    bounded("body", &filing.body, MAX_BODY_CHARS)?;
    if let Some(key) = &filing.fingerprint_key {
        bounded("fingerprint", key, MAX_FINGERPRINT_KEY_CHARS)?;
    }
    let client = match filing.client {
        serde_json::Value::Null => serde_json::Value::Object(serde_json::Map::new()),
        value @ serde_json::Value::Object(_) => value,
        _ => {
            return Err(IssuesError::Invalid {
                field: "client",
                reason: "must be an object".to_owned(),
            });
        },
    };
    if client.to_string().len() > MAX_CLIENT_BYTES {
        return Err(IssuesError::Invalid {
            field: "client",
            reason: format!("at most {MAX_CLIENT_BYTES} bytes"),
        });
    }
    let fingerprint = fingerprint(
        filing.kind,
        filing.via_agent.as_deref(),
        filing.fingerprint_key.as_deref().unwrap_or(&title),
    );
    Ok(NewIssue {
        reporter: filing.reporter,
        via_agent: filing.via_agent,
        kind: filing.kind,
        severity: filing.severity,
        title,
        body: filing.body,
        fingerprint,
        context: filing.context,
        client,
    })
}

#[must_use]
pub fn fingerprint(kind: IssueKind, via_agent: Option<&str>, key: &str) -> String {
    let normalised: String = key
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(via_agent.unwrap_or("").as_bytes());
    hasher.update([0]);
    hasher.update(normalised.as_bytes());
    hex::encode(hasher.finalize())
}
