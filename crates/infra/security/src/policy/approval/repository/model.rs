//! Approval request types and argument-digest helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{CallId, SessionId, UserId};

/// Lifecycle of a held call. Bound to the `approval_requests.status` column,
/// which carries a matching SQL CHECK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

impl ApprovalStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Expired => "expired",
        }
    }

    #[must_use]
    pub const fn is_resolved(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A held call, as the console renders it and the waiter reads it back.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalRequest {
    pub call_id: String,
    pub tool_name: String,
    pub server_name: String,
    // JSON: the tool arguments verbatim, so the approver authorises exactly
    // what will run rather than a re-rendered summary of it.
    pub arguments: serde_json::Value,
    pub args_digest: String,
    pub requested_by: String,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub rule: String,
    pub status: ApprovalStatus,
    pub approver_id: Option<String>,
    pub approver_username: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// What the enforcement point knows when it parks a call.
#[derive(Debug, Clone)]
pub struct NewApprovalRequest<'a> {
    pub call_id: &'a CallId,
    pub tool_name: &'a str,
    pub server_name: &'a str,
    pub arguments: &'a serde_json::Value,
    pub requested_by: &'a UserId,
    pub session_id: Option<&'a SessionId>,
    pub trace_id: Option<&'a str>,
    pub rule: &'a str,
    pub expires_in_seconds: u64,
}

/// One human's answer to a held call.
///
/// Grouped rather than passed loose because the four fields are only ever
/// meaningful together: a status without an approver is not a decision.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalVerdict<'a> {
    pub status: ApprovalStatus,
    pub approver_id: &'a UserId,
    pub approver_username: &'a str,
    pub note: Option<&'a str>,
}

#[must_use]
pub fn args_digest(arguments: &serde_json::Value) -> String {
    let canonical = canonicalize(arguments);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)]);
        out.push(DIGITS[usize::from(byte & 0x0f)]);
    }
    out
}

fn canonicalize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            // Why: BTreeMap ordering: serde_json preserves insertion order unless
            // the `preserve_order` feature is off, so sort explicitly rather
            // than relying on which way that feature happens to be set.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonicalize(&map[k])
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{inner}}}")
        },
        serde_json::Value::Array(items) => {
            let inner = items.iter().map(canonicalize).collect::<Vec<_>>().join(",");
            format!("[{inner}]")
        },
        other => other.to_string(),
    }
}
