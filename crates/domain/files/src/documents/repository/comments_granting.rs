//! The comment writes that also share the document: a comment (or an edit
//! of one) that mentions users without access grants each of them `comment`
//! on the document in the same statement, so the comment never lands
//! without its grants or the grants without the comment. Each grant writes
//! its audit row with the comment as the reason and the agent that wrote
//! the comment, when one did.
//!
//! A grant row that already names the user is renewed at `comment` only
//! when it has expired, and audited as a change from its old level; a live
//! row is left as it is
//! and writes no audit row. The agent is its own audit column, never part
//! of the reason text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};

use super::comments::{CommentRow, DocumentCommentRepository};
use super::rows::RepoResult;
use crate::documents::models::{DocumentComment, MentionGrants, NewComment};

fn ids(users: &[UserId]) -> Vec<String> {
    users.iter().map(|user| user.as_str().to_owned()).collect()
}

impl DocumentCommentRepository {
    pub async fn insert_granting(
        &self,
        request: NewComment,
        grants: &MentionGrants,
    ) -> RepoResult<DocumentComment> {
        let (start, end) = request.range.map(|range| (range.start, range.end)).unzip();
        let mentions = ids(&request.mentions);
        let grantees = ids(&grants.users);
        let row = sqlx::query_as!(
            CommentRow,
            r#"WITH c AS (
                   INSERT INTO document_comments
                       (document_id, node_id, anchor_find, anchor_nth, anchor_start, anchor_end,
                        author_user_id, body, mentions, parent_id, via_agent)
                   VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10::text::uuid, $11)
                   RETURNING *),
               prev AS (
                   SELECT subject_value, access_level FROM document_grants
                   WHERE document_id = $1::text::uuid AND folder_id IS NULL
                     AND subject_kind = 'user' AND subject_value = ANY($13::text[])),
               g AS (
                   INSERT INTO document_grants
                       (document_id, subject_kind, subject_value, access_level, granted_by)
                   SELECT $1::text::uuid, 'user', grantee, 'comment', $12
                   FROM unnest($13::text[]) AS grantee
                   ON CONFLICT (document_id, folder_id, subject_kind, subject_value) DO UPDATE
                       SET access_level = 'comment', valid_until = NULL,
                           granted_by = EXCLUDED.granted_by, updated_at = now()
                       WHERE document_grants.valid_until IS NOT NULL
                         AND document_grants.valid_until <= now()
                   RETURNING *),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        new_level, actor_user_id, trace_id, reason, via_agent)
                   SELECT CASE WHEN prev.access_level IS NULL THEN 'granted' ELSE 'changed' END,
                          'document', g.document_id, g.subject_kind, g.subject_value,
                          prev.access_level, g.access_level, $12, $14,
                          'mentioned in comment ' || (SELECT c.id::text FROM c), $11
                   FROM g LEFT JOIN prev ON prev.subject_value = g.subject_value)
               SELECT c.id::text AS "id!: DocumentCommentId",
                      c.document_id::text AS "document_id!: DocumentId",
                      c.node_id AS "node_id!: DocumentNodeId",
                      c.anchor_find, c.anchor_nth, c.anchor_start, c.anchor_end,
                      c.author_user_id AS "author_user_id!: UserId",
                      c.body AS "body!", c.mentions AS "mentions!",
                      c.parent_id::text AS "parent_id: DocumentCommentId",
                      c.resolved_at, c.resolved_by AS "resolved_by: UserId",
                      c.answering_revision_id::text AS "answering_revision_id: DocumentRevisionId",
                      c.created_at AS "created_at!", c.updated_at AS "updated_at!",
                      c.edited_at, c.deleted_at
               FROM c"#,
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
            grants.via_agent,
            grants.granted_by.as_str(),
            &grantees,
            grants.trace_id.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(row.into())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one statement writes the comment's body, mentions and grants together, so each is its own bound value"
    )]
    pub async fn update_body_granting(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
        body: &str,
        mentions: &[UserId],
        grants: &MentionGrants,
    ) -> RepoResult<bool> {
        let mentions = ids(mentions);
        let grantees = ids(&grants.users);
        let updated = sqlx::query_scalar!(
            r#"WITH c AS (
                   UPDATE document_comments
                   SET body = $4, mentions = $5, edited_at = now(), updated_at = now()
                   WHERE id = $1::text::uuid AND document_id = $2::text::uuid
                     AND author_user_id = $3 AND deleted_at IS NULL
                   RETURNING id, document_id),
               prev AS (
                   SELECT subject_value, access_level FROM document_grants
                   WHERE document_id = $2::text::uuid AND folder_id IS NULL
                     AND subject_kind = 'user' AND subject_value = ANY($6::text[])),
               g AS (
                   INSERT INTO document_grants
                       (document_id, subject_kind, subject_value, access_level, granted_by)
                   SELECT c.document_id, 'user', grantee, 'comment', $3
                   FROM c CROSS JOIN unnest($6::text[]) AS grantee
                   ON CONFLICT (document_id, folder_id, subject_kind, subject_value) DO UPDATE
                       SET access_level = 'comment', valid_until = NULL,
                           granted_by = EXCLUDED.granted_by, updated_at = now()
                       WHERE document_grants.valid_until IS NOT NULL
                         AND document_grants.valid_until <= now()
                   RETURNING *),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        new_level, actor_user_id, trace_id, reason, via_agent)
                   SELECT CASE WHEN prev.access_level IS NULL THEN 'granted' ELSE 'changed' END,
                          'document', g.document_id, g.subject_kind, g.subject_value,
                          prev.access_level, g.access_level, $3, $7,
                          'mentioned in comment ' || $1, $8
                   FROM g LEFT JOIN prev ON prev.subject_value = g.subject_value)
               SELECT COUNT(*) AS "updated!" FROM c"#,
            comment.as_str(),
            document.as_str(),
            grants.granted_by.as_str(),
            body,
            &mentions,
            &grantees,
            grants.trace_id.as_str(),
            grants.via_agent,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(updated == 1)
    }
}
