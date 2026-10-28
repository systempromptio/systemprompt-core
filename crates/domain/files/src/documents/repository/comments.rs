//! [`DocumentCommentRepository`]: comment threads anchored to stable node
//! ids, their resolution, and the revision that answered them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{CommentAnchor, DocumentComment, NewComment};

#[derive(Debug, Clone)]
pub struct DocumentCommentRepository {
    pools: Pools,
}

struct CommentRow {
    id: DocumentCommentId,
    document_id: DocumentId,
    node_id: DocumentNodeId,
    anchor_find: Option<String>,
    anchor_nth: Option<i32>,
    author_user_id: UserId,
    body: String,
    parent_id: Option<DocumentCommentId>,
    resolved_at: Option<DateTime<Utc>>,
    resolved_by: Option<UserId>,
    answering_revision_id: Option<DocumentRevisionId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CommentRow> for DocumentComment {
    fn from(row: CommentRow) -> Self {
        Self {
            id: row.id,
            document_id: row.document_id,
            anchor: CommentAnchor {
                node: row.node_id,
                find: row.anchor_find,
                nth: row.anchor_nth,
            },
            author_user_id: row.author_user_id,
            body: row.body,
            parent_id: row.parent_id,
            resolved_at: row.resolved_at,
            resolved_by: row.resolved_by,
            answering_revision_id: row.answering_revision_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl DocumentCommentRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewComment) -> RepoResult<DocumentComment> {
        let row = sqlx::query_as!(
            CommentRow,
            r#"INSERT INTO document_comments
                   (document_id, node_id, anchor_find, anchor_nth, author_user_id, body, parent_id)
               VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7::text::uuid)
               RETURNING id::text AS "id!: DocumentCommentId",
                         document_id::text AS "document_id!: DocumentId",
                         node_id AS "node_id: DocumentNodeId",
                         anchor_find, anchor_nth,
                         author_user_id AS "author_user_id: UserId",
                         body, parent_id::text AS "parent_id: DocumentCommentId",
                         resolved_at, resolved_by AS "resolved_by: UserId",
                         answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                         created_at, updated_at"#,
            request.document_id.as_str(),
            request.anchor.node.as_str(),
            request.anchor.find,
            request.anchor.nth,
            request.author_user_id.as_str(),
            request.body,
            request.parent_id.as_ref().map(DocumentCommentId::as_str),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(row.into())
    }

    pub async fn list_for_document(
        &self,
        document: &DocumentId,
    ) -> RepoResult<Vec<DocumentComment>> {
        Ok(sqlx::query_as!(
            CommentRow,
            r#"SELECT id::text AS "id!: DocumentCommentId",
                      document_id::text AS "document_id!: DocumentId",
                      node_id AS "node_id: DocumentNodeId",
                      anchor_find, anchor_nth,
                      author_user_id AS "author_user_id: UserId",
                      body, parent_id::text AS "parent_id: DocumentCommentId",
                      resolved_at, resolved_by AS "resolved_by: UserId",
                      answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                      created_at, updated_at
               FROM document_comments WHERE document_id = $1::text::uuid
               ORDER BY created_at, id"#,
            document.as_str()
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentComment::from)
        .collect())
    }

    pub async fn resolve(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
        by: &UserId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_comments
             SET resolved_at = COALESCE(resolved_at, now()),
                 resolved_by = COALESCE(resolved_by, $3), updated_at = now()
             WHERE id = $1::text::uuid AND document_id = $2::text::uuid",
            comment.as_str(),
            document.as_str(),
            by.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_answering(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
        revision: &DocumentRevisionId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_comments
             SET answering_revision_id = $3::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid AND document_id = $2::text::uuid",
            comment.as_str(),
            document.as_str(),
            revision.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn detach(
        &self,
        document: &DocumentId,
        comments: &[DocumentCommentId],
    ) -> RepoResult<u64> {
        let ids: Vec<String> = comments.iter().map(|id| id.as_str().to_owned()).collect();
        let result = sqlx::query!(
            "UPDATE document_comments
             SET anchor_find = NULL, anchor_nth = NULL, updated_at = now()
             WHERE document_id = $1::text::uuid AND id = ANY($2::text[]::uuid[])",
            document.as_str(),
            &ids,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
