//! Writing revisions: the insert, and marking a revision landed when it
//! becomes the head, both inside the commit transaction. Marking it landed
//! also stamps its Drive feed position from the landing transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentId, DocumentRevisionId, SessionId, TaskId, TraceId, UserId,
};

use super::revisions::{DocumentRevisionRepository, RevisionRow};
use super::rows::RepoResult;
use crate::documents::models::{DocumentRevision, NewRevision};

impl DocumentRevisionRepository {
    pub async fn insert(&self, request: NewRevision) -> RepoResult<DocumentRevision> {
        let mut conn = self.pools.write.acquire().await?;
        Self::insert_in(&mut conn, request).await
    }

    pub async fn insert_in(
        conn: &mut PgConnection,
        request: NewRevision,
    ) -> RepoResult<DocumentRevision> {
        let edit_ops = request
            .edit_ops
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let attribution = &request.attribution;
        let row = sqlx::query_as!(
            RevisionRow,
            r#"INSERT INTO document_revisions
                   (id, document_id, parent_revision_id, content_sha256, author_user_id,
                    actor_kind, origin_kind, origin_id, message, edit_ops,
                    session_id, context_id, trace_id, task_id, on_behalf_of)
               VALUES ($15::text::uuid, $1::text::uuid, $2::text::uuid, $3, $4, $5, $6,
                       $7, $8, $9, $10, $11, $12, $13, $14)
               RETURNING id::text AS "id!: DocumentRevisionId",
                         document_id::text AS "document_id!: DocumentId",
                         parent_revision_id::text AS "parent_revision_id: DocumentRevisionId",
                         content_sha256 AS "content_sha256: ContentHash",
                         author_user_id AS "author_user_id: UserId",
                         actor_kind, origin_kind, origin_id, message, edit_ops,
                         session_id AS "session_id: SessionId",
                         context_id AS "context_id: ContextId",
                         trace_id AS "trace_id: TraceId",
                         task_id AS "task_id: TaskId",
                         on_behalf_of AS "on_behalf_of: UserId",
                         created_at, landed_at"#,
            request.document_id.as_str(),
            request
                .parent_revision_id
                .as_ref()
                .map(DocumentRevisionId::as_str),
            request.content_sha256.as_str(),
            request.author_user_id.as_str(),
            request.actor_kind.as_str(),
            request.origin_kind.as_str(),
            request.origin_id,
            request.message,
            edit_ops,
            attribution.session_id.as_ref().map(SessionId::as_str),
            attribution.context_id.as_ref().map(ContextId::as_str),
            attribution.trace_id.as_ref().map(TraceId::as_str),
            attribution.task_id.as_ref().map(TaskId::as_str),
            attribution.on_behalf_of.as_ref().map(UserId::as_str),
            request.id.as_str(),
        )
        .fetch_one(conn)
        .await?;
        row.try_into()
    }

    pub async fn mark_landed(
        conn: &mut PgConnection,
        id: &DocumentRevisionId,
    ) -> RepoResult<DateTime<Utc>> {
        Ok(sqlx::query_scalar!(
            r#"UPDATE document_revisions SET landed_at = COALESCE(landed_at, now()),
                      landing_xid = COALESCE(landing_xid, pg_current_xact_id()::text::bigint),
                      landing_seq = COALESCE(landing_seq, nextval('document_landing_seq'))
               WHERE id = $1::text::uuid
               RETURNING landed_at AS "landed_at!""#,
            id.as_str(),
        )
        .fetch_one(conn)
        .await?)
    }
}
