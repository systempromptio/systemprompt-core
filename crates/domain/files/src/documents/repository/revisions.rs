//! [`DocumentRevisionRepository`]: immutable revisions and the changes feed.
//!
//! History, the changes feed and counts list landed revisions only: a
//! revision is landed once it has been the head (`landed_at`); a proposed
//! revision, or one whose commit lost the head race, never is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentId, DocumentRevisionId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::documents::EditOp;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentRevision, ListCursor, RevisionAttribution};

#[derive(Debug, Clone)]
pub struct DocumentRevisionRepository {
    pub(super) pools: Pools,
}

pub(super) struct RevisionRow {
    pub(super) id: DocumentRevisionId,
    pub(super) document_id: DocumentId,
    pub(super) parent_revision_id: Option<DocumentRevisionId>,
    pub(super) content_sha256: ContentHash,
    pub(super) author_user_id: UserId,
    pub(super) actor_kind: String,
    pub(super) origin_kind: String,
    pub(super) origin_id: Option<String>,
    pub(super) message: Option<String>,
    // JSON: the JSONB edit_ops column, decoded into EditOp values on read.
    pub(super) edit_ops: Option<serde_json::Value>,
    pub(super) session_id: Option<SessionId>,
    pub(super) context_id: Option<ContextId>,
    pub(super) trace_id: Option<TraceId>,
    pub(super) task_id: Option<TaskId>,
    pub(super) on_behalf_of: Option<UserId>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) landed_at: Option<DateTime<Utc>>,
}

impl TryFrom<RevisionRow> for DocumentRevision {
    type Error = systemprompt_traits::RepositoryError;

    fn try_from(row: RevisionRow) -> RepoResult<Self> {
        let edit_ops = row
            .edit_ops
            .map(serde_json::from_value::<Vec<EditOp>>)
            .transpose()?;
        Ok(Self {
            actor_kind: parse("document_revisions.actor_kind", &row.actor_kind)?,
            origin_kind: parse("document_revisions.origin_kind", &row.origin_kind)?,
            id: row.id,
            document_id: row.document_id,
            parent_revision_id: row.parent_revision_id,
            content_sha256: row.content_sha256,
            author_user_id: row.author_user_id,
            origin_id: row.origin_id,
            message: row.message,
            edit_ops,
            attribution: RevisionAttribution {
                session_id: row.session_id,
                context_id: row.context_id,
                trace_id: row.trace_id,
                task_id: row.task_id,
                on_behalf_of: row.on_behalf_of,
            },
            created_at: row.created_at,
            landed_at: row.landed_at,
        })
    }
}

pub(super) fn convert(rows: Vec<RevisionRow>) -> RepoResult<Vec<DocumentRevision>> {
    rows.into_iter().map(DocumentRevision::try_from).collect()
}

impl DocumentRevisionRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn find(&self, id: &DocumentRevisionId) -> RepoResult<Option<DocumentRevision>> {
        sqlx::query_as!(
            RevisionRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
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
                      created_at, landed_at
               FROM document_revisions WHERE id = $1::text::uuid"#,
            id.as_str()
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentRevision::try_from)
        .transpose()
    }

    pub async fn head(&self, document_id: &DocumentId) -> RepoResult<Option<DocumentRevision>> {
        sqlx::query_as!(
RevisionRow,
r#"SELECT id::text AS "id!: DocumentRevisionId",
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
                      created_at, landed_at
               FROM document_revisions WHERE id = (SELECT head_revision_id FROM documents WHERE id = $1::text::uuid)"#,
            document_id.as_str()
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentRevision::try_from)
        .transpose()
    }

    pub async fn list(
        &self,
        document_id: &DocumentId,
        limit: i64,
        before: Option<ListCursor>,
    ) -> RepoResult<Vec<DocumentRevision>> {
        let rows = sqlx::query_as!(
            RevisionRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
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
                      created_at, landed_at
               FROM document_revisions WHERE document_id = $1::text::uuid
               AND landed_at IS NOT NULL
               AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3::uuid))
             ORDER BY created_at DESC, id DESC
             LIMIT $4"#,
            document_id.as_str(),
            before.map(|c| c.updated_at),
            before.map(|c| c.id),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?;
        convert(rows)
    }

    pub async fn changes_since(
        &self,
        cursor: Option<ListCursor>,
        limit: i64,
    ) -> RepoResult<Vec<DocumentRevision>> {
        let rows = sqlx::query_as!(
            RevisionRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
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
                      created_at, landed_at
               FROM document_revisions WHERE landed_at IS NOT NULL
               AND ($1::timestamptz IS NULL OR (created_at, id) > ($1, $2::uuid))
             ORDER BY created_at, id
             LIMIT $3"#,
            cursor.map(|c| c.updated_at),
            cursor.map(|c| c.id),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?;
        convert(rows)
    }

    pub async fn with_content(
        &self,
        hash: &ContentHash,
        limit: i64,
    ) -> RepoResult<Vec<DocumentRevision>> {
        let rows = sqlx::query_as!(
            RevisionRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
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
                      created_at, landed_at
               FROM document_revisions WHERE content_sha256 = $1
             ORDER BY created_at DESC, id DESC
             LIMIT $2"#,
            hash.as_str(),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?;
        convert(rows)
    }

    pub async fn count(&self, document_id: &DocumentId) -> RepoResult<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM document_revisions WHERE document_id = $1::text::uuid AND landed_at IS NOT NULL"#,
            document_id.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok(count)
    }
}
