//! `approval_requests` reads and writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
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

/// Binds an approval to the exact payload it authorises.
///
/// Why: the approver sees one set of arguments, but MRTR means the client
/// sends the call again to collect the result. Without this, a retry could
/// swap the payload after approval and ride the approved row. The digest is
/// taken over the canonical serialisation, so key order cannot change it.
#[must_use]
pub fn args_digest(arguments: &serde_json::Value) -> String {
    let canonical = canonicalize(arguments);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex(&hasher.finalize())
}

/// Lowercase hex of a digest. `GenericArray` has no `LowerHex`, and pulling a
/// hex crate in for sixteen characters of formatting is not worth a dependency.
pub(crate) fn hex(bytes: &[u8]) -> String {
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
            // BTreeMap ordering: serde_json preserves insertion order unless
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

#[derive(Debug, Clone)]
pub struct ApprovalRepository {
    pool: PgPool,
}

impl ApprovalRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Opens an approval for a call, or returns the one already open for it.
    ///
    /// `ON CONFLICT DO NOTHING` rather than an upsert: a retried MRTR round
    /// must not reset `expires_at` or wipe a decision that has already been
    /// taken between rounds.
    pub async fn open(&self, req: &NewApprovalRequest<'_>) -> Result<ApprovalRequest, sqlx::Error> {
        let digest = args_digest(req.arguments);
        let expires_at = Utc::now()
            + Duration::try_seconds(i64::try_from(req.expires_in_seconds).unwrap_or(i64::MAX))
                .unwrap_or_else(|| Duration::seconds(900));

        sqlx::query!(
            "INSERT INTO approval_requests (
                 call_id, tool_name, server_name, arguments, args_digest,
                 requested_by, session_id, trace_id, rule, expires_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (call_id) DO NOTHING",
            req.call_id.as_str(),
            req.tool_name,
            req.server_name,
            req.arguments,
            digest,
            req.requested_by.as_str(),
            req.session_id.map(SessionId::as_str),
            req.trace_id,
            req.rule,
            expires_at,
        )
        .execute(&self.pool)
        .await?;

        self.find(req.call_id.as_str())
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find(&self, call_id: &str) -> Result<Option<ApprovalRequest>, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT call_id, tool_name, server_name, arguments, args_digest, requested_by,
                    session_id, trace_id, rule, status, approver_id, approver_username,
                    decided_at, decision_note, expires_at, created_at
             FROM approval_requests WHERE call_id = $1",
            call_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ApprovalRequest {
            call_id: r.call_id,
            tool_name: r.tool_name,
            server_name: r.server_name,
            arguments: r.arguments,
            args_digest: r.args_digest,
            requested_by: r.requested_by,
            session_id: r.session_id,
            trace_id: r.trace_id,
            rule: r.rule,
            status: parse_status(&r.status),
            approver_id: r.approver_id,
            approver_username: r.approver_username,
            decided_at: r.decided_at,
            decision_note: r.decision_note,
            expires_at: r.expires_at,
            created_at: r.created_at,
        }))
    }

    /// Everything still awaiting a human, newest first. Expired-but-unswept
    /// rows are filtered out here so the console never offers a button that
    /// cannot do anything.
    pub async fn list_pending(&self, limit: i64) -> Result<Vec<ApprovalRequest>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT call_id, tool_name, server_name, arguments, args_digest, requested_by,
                    session_id, trace_id, rule, status, approver_id, approver_username,
                    decided_at, decision_note, expires_at, created_at
             FROM approval_requests
             WHERE status = 'pending' AND expires_at > NOW()
             ORDER BY created_at DESC
             LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ApprovalRequest {
                call_id: r.call_id,
                tool_name: r.tool_name,
                server_name: r.server_name,
                arguments: r.arguments,
                args_digest: r.args_digest,
                requested_by: r.requested_by,
                session_id: r.session_id,
                trace_id: r.trace_id,
                rule: r.rule,
                status: parse_status(&r.status),
                approver_id: r.approver_id,
                approver_username: r.approver_username,
                decided_at: r.decided_at,
                decision_note: r.decision_note,
                expires_at: r.expires_at,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Records a human decision. Returns `Ok(None)` when the row was already
    /// resolved or has expired, so a double-click cannot overwrite the first
    /// decision and a late click cannot revive an abandoned call.
    pub async fn resolve(
        &self,
        call_id: &str,
        verdict: &ApprovalVerdict<'_>,
    ) -> Result<Option<ApprovalRequest>, sqlx::Error> {
        let ApprovalVerdict {
            status,
            approver_id,
            approver_username,
            note,
        } = *verdict;
        debug_assert!(
            status.is_resolved(),
            "resolve() cannot set status back to pending"
        );

        let updated = sqlx::query_scalar!(
            "UPDATE approval_requests
             SET status = $2, approver_id = $3, approver_username = $4,
                 decision_note = $5, decided_at = NOW()
             WHERE call_id = $1 AND status = 'pending' AND expires_at > NOW()
             RETURNING call_id",
            call_id,
            status.as_str(),
            approver_id.as_str(),
            approver_username,
            note,
        )
        .fetch_optional(&self.pool)
        .await?;

        match updated {
            Some(_) => self.find(call_id).await,
            None => Ok(None),
        }
    }

    /// Sweeps rows nobody answered in time. Idempotent.
    pub async fn expire_due(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "UPDATE approval_requests
             SET status = 'expired', decided_at = NOW()
             WHERE status = 'pending' AND expires_at <= NOW()"
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

// Why: the column is CHECK-constrained to these four, so an unknown value
// means the constraint was dropped out of band. Treat it as unresolved rather
// than guessing — a held call that stays held is recoverable; one that decodes
// garbage as `Approved` is not.
fn parse_status(raw: &str) -> ApprovalStatus {
    match raw {
        "approved" => ApprovalStatus::Approved,
        "denied" => ApprovalStatus::Denied,
        "expired" => ApprovalStatus::Expired,
        "pending" => ApprovalStatus::Pending,
        other => {
            tracing::error!(
                status = other,
                "approval_requests.status holds a value outside its CHECK constraint; \
                 treating the call as still pending"
            );
            ApprovalStatus::Pending
        },
    }
}
