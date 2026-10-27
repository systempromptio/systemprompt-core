//! [`DocumentRevisionRepository`]: immutable revisions and the changes feed.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId, UserId};
use systemprompt_models::documents::EditOp;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentRevision, ListCursor, NewRevision};

#[derive(Debug, Clone)]
pub struct DocumentRevisionRepository {
    pools: Pools,
}

struct RevisionRow {
    id: DocumentRevisionId,
    document_id: DocumentId,
    parent_revision_id: Option<DocumentRevisionId>,
    content_sha256: ContentHash,
    author_user_id: UserId,
    actor_kind: String,
    origin_kind: String,
    origin_id: Option<String>,
    message: Option<String>,
    edit_ops: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
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
            created_at: row.created_at,
        })
    }
}

fn convert(rows: Vec<RevisionRow>) -> RepoResult<Vec<DocumentRevision>> {
    rows.into_iter().map(DocumentRevision::try_from).collect()
}

impl DocumentRevisionRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewRevision) -> RepoResult<DocumentRevision> {
        let edit_ops = request
            .edit_ops
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let row = sqlx::query_as!(
            RevisionRow,
            r#"INSERT INTO document_revisions
                   (document_id, parent_revision_id, content_sha256, author_user_id,
                    actor_kind, origin_kind, origin_id, message, edit_ops)
               VALUES ($1::text::uuid, $2::text::uuid, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id::text AS "id!: DocumentRevisionId",
                         document_id::text AS "document_id!: DocumentId",
                         parent_revision_id::text AS "parent_revision_id: DocumentRevisionId",
                         content_sha256 AS "content_sha256: ContentHash",
                         author_user_id AS "author_user_id: UserId",
                         actor_kind, origin_kind, origin_id, message, edit_ops, created_at"#,
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
        )
        .fetch_one(&*self.pools.write)
        .await?;
        row.try_into()
    }

    pub async fn find(&self, id: &DocumentRevisionId) -> RepoResult<Option<DocumentRevision>> {
        sqlx::query_as!(
            RevisionRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
                      document_id::text AS "document_id!: DocumentId",
                      parent_revision_id::text AS "parent_revision_id: DocumentRevisionId",
                      content_sha256 AS "content_sha256: ContentHash",
                      author_user_id AS "author_user_id: UserId",
                      actor_kind, origin_kind, origin_id, message, edit_ops, created_at
               FROM document_revisions WHERE id = $1::text::uuid"#,
            id.as_str()
        )
        .fetch_optional(&*self.pools.read)
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
                      actor_kind, origin_kind, origin_id, message, edit_ops, created_at
               FROM document_revisions WHERE id = (SELECT head_revision_id FROM documents WHERE id = $1::text::uuid)"#,
            document_id.as_str()
        )
        .fetch_optional(&*self.pools.read)
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
                      actor_kind, origin_kind, origin_id, message, edit_ops, created_at
               FROM document_revisions WHERE document_id = $1::text::uuid
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
                      actor_kind, origin_kind, origin_id, message, edit_ops, created_at
               FROM document_revisions WHERE ($1::timestamptz IS NULL OR (created_at, id) > ($1, $2::uuid))
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

    pub async fn count(&self, document_id: &DocumentId) -> RepoResult<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM document_revisions WHERE document_id = $1::text::uuid"#,
            document_id.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok(count)
    }
}
