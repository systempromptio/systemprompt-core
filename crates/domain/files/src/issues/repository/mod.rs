//! The issue report tables: filing (folded by fingerprint while a report is
//! active), keyset listing, triage transitions, attachments and retention.
//!
//! Filing is one `INSERT … ON CONFLICT` against the partial unique index on
//! `(reporter_user_id, fingerprint)` over active reports, so a repeat of an
//! open report costs one index probe and bumps `occurrences` instead of
//! adding a row. Attaching locks the report row, so the attachment cap holds
//! under concurrent attaches, and links only a completed upload the reporter
//! owns, in the same statement that reads it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rows;

use std::sync::Arc;

use sqlx::{PgConnection, PgPool};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentUploadId, IssueReportId, McpExecutionId, SessionId, TaskId,
    TraceId, UserId,
};
use systemprompt_models::issues::{IssueAttachment, IssueReport, IssueStatus};

pub use self::rows::{Filed, IssueListQuery, NewIssue, StatusChange};
use self::rows::{RepoResult, ReportRow, parse};

#[derive(Debug, Clone)]
pub struct IssueReportRepository {
    read: Arc<PgPool>,
    write: Arc<PgPool>,
}

impl IssueReportRepository {
    #[must_use]
    pub fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }

    #[must_use]
    pub fn write_pool(&self) -> &PgPool {
        &self.write
    }

    pub async fn filed_last_hour(conn: &mut PgConnection, reporter: &UserId) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM issue_reports
               WHERE reporter_user_id = $1 AND created_at > now() - interval '1 hour'"#,
            reporter.as_str(),
        )
        .fetch_one(&mut *conn)
        .await?)
    }

    pub async fn file(
        conn: &mut PgConnection,
        issue: &NewIssue,
    ) -> RepoResult<(IssueReportId, Filed)> {
        let row = sqlx::query!(
            r#"INSERT INTO issue_reports
                   (reporter_user_id, via_agent, kind, severity, title, body, fingerprint,
                    trace_id, context_id, session_id, task_id, mcp_execution_id, client)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
               ON CONFLICT (reporter_user_id, fingerprint)
                   WHERE status IN ('open', 'triaged', 'needs_info')
               DO UPDATE SET
                   occurrences = issue_reports.occurrences + 1,
                   last_seen_at = now(),
                   updated_at = now(),
                   severity = CASE
                       WHEN array_position(ARRAY['low','medium','high','critical'], EXCLUDED.severity)
                          > array_position(ARRAY['low','medium','high','critical'], issue_reports.severity)
                       THEN EXCLUDED.severity ELSE issue_reports.severity END,
                   trace_id = EXCLUDED.trace_id,
                   context_id = EXCLUDED.context_id,
                   session_id = EXCLUDED.session_id,
                   task_id = EXCLUDED.task_id,
                   mcp_execution_id = EXCLUDED.mcp_execution_id
               RETURNING id::text AS "id!: IssueReportId", (xmax = 0) AS "inserted!",
                         occurrences"#,
            issue.reporter.as_str(),
            issue.via_agent,
            issue.kind.as_str(),
            issue.severity.as_str(),
            issue.title,
            issue.body,
            issue.fingerprint,
            issue.context.trace.as_ref().map(TraceId::as_str),
            issue.context.context.as_ref().map(ContextId::as_str),
            issue.context.session.as_ref().map(SessionId::as_str),
            issue.context.task.as_ref().map(TaskId::as_str),
            issue.context.mcp_execution.as_ref().map(McpExecutionId::as_str),
            issue.client,
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok((
            row.id,
            Filed {
                inserted: row.inserted,
                occurrences: row.occurrences,
            },
        ))
    }

    pub async fn find(&self, id: &IssueReportId) -> RepoResult<Option<IssueReport>> {
        let row = sqlx::query_as!(
            ReportRow,
            r#"SELECT id::text AS "id!: IssueReportId",
                      reporter_user_id AS "reporter_user_id!: UserId", via_agent, kind, severity,
                      status, title, body, occurrences, trace_id AS "trace_id?: TraceId",
                      context_id AS "context_id?: ContextId",
                      session_id AS "session_id?: SessionId", task_id AS "task_id?: TaskId",
                      mcp_execution_id AS "mcp_execution_id?: McpExecutionId", client,
                      triage_note, triaged_by AS "triaged_by?: UserId",
                      created_at, last_seen_at, updated_at, closed_at
               FROM issue_reports WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(&*self.read)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut report = row.into_report()?;
        report.attachments = self.attachments(id).await?;
        Ok(Some(report))
    }

    pub async fn attachments(&self, id: &IssueReportId) -> RepoResult<Vec<IssueAttachment>> {
        Ok(sqlx::query_as!(
            IssueAttachment,
            r#"SELECT upload_id::text AS "upload_id!: DocumentUploadId",
                      content_sha256 AS "content_sha256!: ContentHash",
                      file_name, mime_type, size_bytes, created_at
               FROM issue_report_attachments WHERE report_id = $1::text::uuid
               ORDER BY created_at, upload_id"#,
            id.as_str(),
        )
        .fetch_all(&*self.read)
        .await?)
    }

    pub async fn list(&self, query: &IssueListQuery) -> RepoResult<Vec<IssueReport>> {
        let (before_at, before_id) = query
            .before
            .as_ref()
            .map_or((None, None), |(at, id)| (Some(*at), Some(id.as_str())));
        sqlx::query_as!(
            ReportRow,
            r#"SELECT id::text AS "id!: IssueReportId",
                      reporter_user_id AS "reporter_user_id!: UserId", via_agent, kind, severity,
                      status, title, body, occurrences, trace_id AS "trace_id?: TraceId",
                      context_id AS "context_id?: ContextId",
                      session_id AS "session_id?: SessionId", task_id AS "task_id?: TaskId",
                      mcp_execution_id AS "mcp_execution_id?: McpExecutionId", client,
                      triage_note, triaged_by AS "triaged_by?: UserId",
                      created_at, last_seen_at, updated_at, closed_at
               FROM issue_reports
               WHERE ($1::text IS NULL OR reporter_user_id = $1)
                 AND ($2::text IS NULL OR status = $2)
                 AND ($3::timestamptz IS NULL OR (last_seen_at, id) < ($3, $4::text::uuid))
               ORDER BY last_seen_at DESC, id DESC
               LIMIT $5"#,
            query.reporter.as_ref().map(UserId::as_str),
            query.status.map(IssueStatus::as_str),
            before_at,
            before_id,
            i64::from(query.limit.clamp(1, 200)),
        )
        .fetch_all(&*self.read)
        .await?
        .into_iter()
        .map(ReportRow::into_report)
        .collect()
    }

    pub async fn set_status(
        conn: &mut PgConnection,
        id: &IssueReportId,
        status: IssueStatus,
        note: Option<&str>,
        by: &UserId,
    ) -> RepoResult<Option<StatusChange>> {
        let row = sqlx::query!(
            r#"UPDATE issue_reports r
               SET status = $2, triage_note = COALESCE($3, r.triage_note), triaged_by = $4,
                   updated_at = now(),
                   closed_at = CASE WHEN $2 IN ('resolved', 'closed')
                                    THEN COALESCE(r.closed_at, now()) ELSE NULL END
               FROM (SELECT id, status FROM issue_reports WHERE id = $1::text::uuid FOR UPDATE) prior
               WHERE r.id = prior.id
               RETURNING r.reporter_user_id AS "reporter!: UserId", prior.status AS "from!",
                         r.title"#,
            id.as_str(),
            status.as_str(),
            note,
            by.as_str(),
        )
        .fetch_optional(&mut *conn)
        .await?;
        row.map(|row| {
            Ok(StatusChange {
                reporter: row.reporter,
                from: parse("issue_reports.status", &row.from)?,
                title: row.title,
            })
        })
        .transpose()
    }

    pub async fn lock_active(
        conn: &mut PgConnection,
        id: &IssueReportId,
        reporter: &UserId,
    ) -> RepoResult<Option<(bool, i64)>> {
        let row = sqlx::query!(
            r#"SELECT status IN ('open', 'triaged', 'needs_info') AS "active!",
                      (SELECT COUNT(*) FROM issue_report_attachments a
                       WHERE a.report_id = r.id) AS "attachments!"
               FROM issue_reports r
               WHERE r.id = $1::text::uuid AND r.reporter_user_id = $2
               FOR UPDATE"#,
            id.as_str(),
            reporter.as_str(),
        )
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|row| (row.active, row.attachments)))
    }

    pub async fn attach_completed_upload(
        conn: &mut PgConnection,
        id: &IssueReportId,
        upload: &DocumentUploadId,
        reporter: &UserId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            r#"INSERT INTO issue_report_attachments
                   (report_id, upload_id, content_sha256, file_name, mime_type, size_bytes)
               SELECT $1::text::uuid, u.id, u.blob_sha256, u.file_name,
                      COALESCE(u.detected_mime, u.declared_mime), u.declared_size
               FROM document_uploads u
               WHERE u.id = $2::text::uuid AND u.owner_user_id = $3 AND u.status = 'completed'
                 AND u.blob_sha256 IS NOT NULL
               ON CONFLICT DO NOTHING"#,
            id.as_str(),
            upload.as_str(),
            reporter.as_str(),
        )
        .execute(&mut *conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn purge_closed(&self, retention_days: u32, limit: i64) -> RepoResult<u64> {
        Ok(sqlx::query!(
            "DELETE FROM issue_reports WHERE id IN (
                 SELECT id FROM issue_reports
                 WHERE closed_at < now() - make_interval(days => $1::int)
                 ORDER BY closed_at LIMIT $2)",
            i32::try_from(retention_days).unwrap_or(i32::MAX),
            limit.max(1),
        )
        .execute(&*self.write)
        .await?
        .rows_affected())
    }
}
