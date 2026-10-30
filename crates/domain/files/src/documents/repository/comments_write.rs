//! The writes to a comment after insertion: resolving it, recording the
//! revision that answered it, detaching quotes from deleted text, editing its
//! body and mentions (its author only), and soft deletion.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentRevisionId, UserId};

use super::comments::DocumentCommentRepository;
use super::rows::RepoResult;

#[derive(Debug, Clone, Copy)]
pub struct CommentBodyEdit<'a> {
    pub comment: &'a DocumentCommentId,
    pub author: &'a UserId,
    pub body: &'a str,
    pub mentions: &'a [UserId],
}

impl DocumentCommentRepository {
    pub async fn resolve_in(
        conn: &mut PgConnection,
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
        .execute(&mut *conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_answering_in(
        conn: &mut PgConnection,
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
        .execute(&mut *conn)
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

    pub async fn update_body(
        &self,
        conn: &mut PgConnection,
        document: &DocumentId,
        edit: CommentBodyEdit<'_>,
    ) -> RepoResult<bool> {
        let CommentBodyEdit {
            comment,
            author,
            body,
            mentions,
        } = edit;
        let mentions: Vec<String> = mentions
            .iter()
            .map(|user| user.as_str().to_owned())
            .collect();
        let result = sqlx::query!(
            "UPDATE document_comments
             SET body = $4, mentions = $5, edited_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND document_id = $2::text::uuid
               AND author_user_id = $3 AND deleted_at IS NULL",
            comment.as_str(),
            document.as_str(),
            author.as_str(),
            body,
            &mentions,
        )
        .execute(&mut *conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn soft_delete(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_comments
             SET deleted_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND document_id = $2::text::uuid AND deleted_at IS NULL",
            comment.as_str(),
            document.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
