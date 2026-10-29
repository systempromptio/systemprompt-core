//! Path-changing writes on [`DocumentRepository`] and the path history the
//! bridge Drive change feed reads.
//!
//! Every trash, restore, rename and move writes its `document_path_changes`
//! row in the same statement as the document update, so the history never
//! disagrees with the document row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};

use super::documents::DocumentRepository;
use super::feed::FeedHorizon;
use super::rows::RepoResult;
use crate::documents::models::DriveCursor;

/// The path a document had and has after one trash, restore, rename or move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMove<'a> {
    pub from: &'a str,
    pub to: &'a str,
}

/// One row of the path history: `from` is absent for a restore and `to`
/// for a trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPathChange {
    pub cursor: DriveCursor,
    pub document_id: DocumentId,
    pub from_path: Option<String>,
    pub to_path: Option<String>,
}

impl DocumentRepository {
    pub async fn rename(
        &self,
        id: &DocumentId,
        name: &str,
        paths: PathMove<'_>,
    ) -> RepoResult<bool> {
        let updated = sqlx::query_scalar!(
            r#"WITH moved AS (
                   UPDATE documents SET name = $2, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, from_path, to_path)
                   SELECT id, $3, $4 FROM moved WHERE $3 <> $4 RETURNING 1)
               SELECT COUNT(*) AS "updated!" FROM moved"#,
            id.as_str(),
            name,
            paths.from,
            paths.to,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(updated == 1)
    }

    pub async fn move_to(
        &self,
        id: &DocumentId,
        folder: Option<&DocumentFolderId>,
        paths: PathMove<'_>,
    ) -> RepoResult<bool> {
        let folders: Vec<&str> = folder.map(DocumentFolderId::as_str).into_iter().collect();
        let mut tx = super::folder_locks::begin_tree_locked(&self.pools.write, &folders).await?;
        let updated = sqlx::query_scalar!(
            r#"WITH moved AS (
                   UPDATE documents SET folder_id = $2::text::uuid, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL
                     AND ($2::text IS NULL OR EXISTS (
                         SELECT 1 FROM document_folders f
                         WHERE f.id = $2::text::uuid AND f.deleted_at IS NULL))
                   RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, from_path, to_path)
                   SELECT id, $3, $4 FROM moved WHERE $3 <> $4 RETURNING 1)
               SELECT COUNT(*) AS "updated!" FROM moved"#,
            id.as_str(),
            folder.map(DocumentFolderId::as_str),
            paths.from,
            paths.to,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(updated == 1)
    }

    pub async fn trash(&self, id: &DocumentId, by: &UserId, from_path: &str) -> RepoResult<bool> {
        let updated = sqlx::query_scalar!(
            r#"WITH trashed AS (
                   UPDATE documents SET deleted_at = now(), trashed_by = $2, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, from_path)
                   SELECT id, $3 FROM trashed RETURNING 1)
               SELECT COUNT(*) AS "updated!" FROM trashed"#,
            id.as_str(),
            by.as_str(),
            from_path,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(updated == 1)
    }

    pub async fn restore(&self, id: &DocumentId, to_path: &str) -> RepoResult<bool> {
        let updated = sqlx::query_scalar!(
            r#"WITH restored AS (
                   UPDATE documents SET deleted_at = NULL, trashed_by = NULL, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NOT NULL RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, to_path)
                   SELECT id, $2 FROM restored RETURNING 1)
               SELECT COUNT(*) AS "updated!" FROM restored"#,
            id.as_str(),
            to_path,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(updated == 1)
    }

    pub async fn path_changes_since(
        &self,
        after: DriveCursor,
        horizon: FeedHorizon,
        limit: i64,
    ) -> RepoResult<Vec<DocumentPathChange>> {
        let rows = sqlx::query!(
            r#"SELECT document_id::text AS "document_id!: DocumentId",
                      from_path, to_path,
                      landing_xid AS "landing_xid!", landing_seq AS "landing_seq!"
               FROM document_path_changes
               WHERE landing_xid IS NOT NULL
                 AND landing_xid < $3
                 AND (landing_xid, landing_seq) > ($1, $2)
               ORDER BY landing_xid, landing_seq
               LIMIT $4"#,
            after.xid,
            after.seq,
            horizon.0,
            limit.max(1),
        )
        .fetch_all(&*self.pools.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DocumentPathChange {
                cursor: DriveCursor {
                    xid: row.landing_xid,
                    seq: row.landing_seq,
                },
                document_id: row.document_id,
                from_path: row.from_path,
                to_path: row.to_path,
            })
            .collect())
    }
}
