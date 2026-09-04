//! `approval_requests` reads and writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod model;

use chrono::{Duration, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

pub use model::{
    ApprovalRequest, ApprovalStatus, ApprovalVerdict, NewApprovalRequest, args_digest,
};

#[derive(Debug, Clone)]
pub struct ApprovalRepository {
    pool: PgPool,
}

impl ApprovalRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

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

    // Why: the decided rows are the audit half of the queue — an approvals
    // console that shows only what is still pending cannot answer "who let
    // that through?". Expired rows come back too: nobody decided them, and
    // that is itself the answer.
    pub async fn list_decided(&self, limit: i64) -> Result<Vec<ApprovalRequest>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT call_id, tool_name, server_name, arguments, args_digest, requested_by,
                    session_id, trace_id, rule, status, approver_id, approver_username,
                    decided_at, decision_note, expires_at, created_at
             FROM approval_requests
             WHERE status <> 'pending'
             ORDER BY decided_at DESC
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
