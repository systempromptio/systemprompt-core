//! [`DocumentCommentRepository`]: comment threads anchored to stable node
//! ids and, optionally, a range of engine cursors or a cell range; their
//! mentions, edits and soft deletion, their resolution and reopening, and the
//! revision that answered them. The
//! writes after insertion live in `comments_write`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AgentName, ClientId, ContextId, DocumentCommentId, DocumentId, DocumentNodeId,
    DocumentRevisionId, UserId,
};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{
    CellRange, CommentAnchor, CommentRange, DocumentComment, NewComment,
};

#[derive(Debug, Clone)]
pub struct DocumentCommentRepository {
    pub(super) pools: Pools,
}

pub(super) struct CommentRow {
    pub(super) id: DocumentCommentId,
    pub(super) document_id: DocumentId,
    pub(super) node_id: DocumentNodeId,
    pub(super) anchor_find: Option<String>,
    pub(super) anchor_nth: Option<i32>,
    pub(super) anchor_start: Option<String>,
    pub(super) anchor_end: Option<String>,
    pub(super) anchor_cells: Option<String>,
    pub(super) author_user_id: UserId,
    pub(super) body: String,
    pub(super) mentions: Vec<String>,
    pub(super) parent_id: Option<DocumentCommentId>,
    pub(super) resolved_at: Option<DateTime<Utc>>,
    pub(super) resolved_by: Option<UserId>,
    pub(super) reopened_at: Option<DateTime<Utc>>,
    pub(super) reopened_by: Option<UserId>,
    pub(super) answering_revision_id: Option<DocumentRevisionId>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) edited_at: Option<DateTime<Utc>>,
    pub(super) deleted_at: Option<DateTime<Utc>>,
    pub(super) via_agent: Option<AgentName>,
    pub(super) context_id: Option<ContextId>,
    pub(super) client_id: Option<ClientId>,
}

fn stored_cells(text: &str) -> Option<CellRange> {
    text.parse()
        .map_err(
            |error| tracing::warn!(cells = text, %error, "Stored comment cell range is unreadable"),
        )
        .ok()
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
                cells: row.anchor_cells.as_deref().and_then(stored_cells),
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
            reopened_at: row.reopened_at,
            reopened_by: row.reopened_by,
            answering_revision_id: row.answering_revision_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            edited_at: row.edited_at,
            deleted_at: row.deleted_at,
            via_agent: row.via_agent,
            context_id: row.context_id,
            client_id: row.client_id,
        }
    }
}

impl DocumentCommentRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(
        &self,
        conn: &mut PgConnection,
        request: NewComment,
    ) -> RepoResult<DocumentComment> {
        let (start, end) = request.range.map(|range| (range.start, range.end)).unzip();
        let cells = request.anchor.cells.map(|cells| cells.to_string());
        let mentions: Vec<String> = request
            .mentions
            .iter()
            .map(|user| user.as_str().to_owned())
            .collect();
        let row = sqlx::query_as!(
            CommentRow,
            r#"INSERT INTO document_comments
                   (document_id, node_id, anchor_find, anchor_nth, anchor_start, anchor_end,
                    author_user_id, body, mentions, parent_id, via_agent, context_id,
                    client_id, anchor_cells)
               VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10::text::uuid, $11,
                       $12, $13, $14)
               RETURNING id::text AS "id!: DocumentCommentId",
                         document_id::text AS "document_id!: DocumentId",
                         node_id AS "node_id: DocumentNodeId",
                         anchor_find, anchor_nth, anchor_start, anchor_end, anchor_cells,
                         author_user_id AS "author_user_id: UserId",
                         body, mentions, parent_id::text AS "parent_id: DocumentCommentId",
                         resolved_at, resolved_by AS "resolved_by: UserId",
                         reopened_at, reopened_by AS "reopened_by: UserId",
                         answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                         created_at, updated_at, edited_at, deleted_at,
                         via_agent AS "via_agent: AgentName",
                         context_id AS "context_id: ContextId",
                         client_id AS "client_id: ClientId""#,
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
            request.via_agent.as_ref().map(AgentName::as_str),
            request.context_id.as_ref().map(ContextId::as_str),
            request.client_id.as_ref().map(ClientId::as_str),
            cells,
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(row.into())
    }

    pub async fn find_live_document_of(
        &self,
        comment: &DocumentCommentId,
    ) -> RepoResult<Option<DocumentId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT document_id::text AS "document_id!: DocumentId"
               FROM document_comments
               WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
            comment.as_str()
        )
        .fetch_optional(&*self.pools.read)
        .await?)
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
                      anchor_find, anchor_nth, anchor_start, anchor_end, anchor_cells,
                      author_user_id AS "author_user_id: UserId",
                      body, mentions, parent_id::text AS "parent_id: DocumentCommentId",
                      resolved_at, resolved_by AS "resolved_by: UserId",
                      reopened_at, reopened_by AS "reopened_by: UserId",
                      answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                      created_at, updated_at, edited_at, deleted_at,
                      via_agent AS "via_agent: AgentName",
                      context_id AS "context_id: ContextId",
                      client_id AS "client_id: ClientId"
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
