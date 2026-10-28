//! [`DocumentCommentRepository`]: comment threads anchored to stable node
//! ids and, optionally, a range of engine cursors; their mentions, edits and
//! soft deletion, their resolution, and the revision that answered them. The
//! writes after insertion live in `comments_write`.
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
use crate::documents::models::{CommentAnchor, CommentRange, DocumentComment, NewComment};

#[derive(Debug, Clone)]
pub struct DocumentCommentRepository {
    pub(super) pools: Pools,
}

struct CommentRow {
    id: DocumentCommentId,
    document_id: DocumentId,
    node_id: DocumentNodeId,
    anchor_find: Option<String>,
    anchor_nth: Option<i32>,
    anchor_start: Option<String>,
    anchor_end: Option<String>,
    author_user_id: UserId,
    body: String,
    mentions: Vec<String>,
    parent_id: Option<DocumentCommentId>,
    resolved_at: Option<DateTime<Utc>>,
    resolved_by: Option<UserId>,
    answering_revision_id: Option<DocumentRevisionId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
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
            range: row
                .anchor_start
                .zip(row.anchor_end)
                .map(|(start, end)| CommentRange { start, end }),
            author_user_id: row.author_user_id,
            body: row.body,
            mentions: row.mentions.into_iter().map(UserId::new).collect(),
            parent_id: row.parent_id,
            resolved_at: row.resolved_at,
            resolved_by: row.resolved_by,
            answering_revision_id: row.answering_revision_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            edited_at: row.edited_at,
            deleted_at: row.deleted_at,
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
        let (start, end) = request.range.map(|range| (range.start, range.end)).unzip();
        let mentions: Vec<String> = request
            .mentions
            .iter()
            .map(|user| user.as_str().to_owned())
            .collect();
        let row = sqlx::query_as!(
            CommentRow,
            r#"INSERT INTO document_comments
                   (document_id, node_id, anchor_find, anchor_nth, anchor_start, anchor_end,
                    author_user_id, body, mentions, parent_id)
               VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10::text::uuid)
               RETURNING id::text AS "id!: DocumentCommentId",
                         document_id::text AS "document_id!: DocumentId",
                         node_id AS "node_id: DocumentNodeId",
                         anchor_find, anchor_nth, anchor_start, anchor_end,
                         author_user_id AS "author_user_id: UserId",
                         body, mentions, parent_id::text AS "parent_id: DocumentCommentId",
                         resolved_at, resolved_by AS "resolved_by: UserId",
                         answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                         created_at, updated_at, edited_at, deleted_at"#,
            request.document_id.as_str(),
            request.anchor.node.as_str(),
            request.anchor.find,
            request.anchor.nth,
            start,
            end,
            request.author_user_id.as_str(),
            request.body,
            &mentions,
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
                      anchor_find, anchor_nth, anchor_start, anchor_end,
                      author_user_id AS "author_user_id: UserId",
                      body, mentions, parent_id::text AS "parent_id: DocumentCommentId",
                      resolved_at, resolved_by AS "resolved_by: UserId",
                      answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                      created_at, updated_at, edited_at, deleted_at
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
}
