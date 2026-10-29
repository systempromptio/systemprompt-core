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
use super::rows::RepoResult;
use crate::documents::models::ListCursor;

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
    pub cursor: ListCursor,
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
        let updated = sqlx::query_scalar!(
            r#"WITH moved AS (
                   UPDATE documents SET folder_id = $2::text::uuid, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, from_path, to_path)
                   SELECT id, $3, $4 FROM moved WHERE $3 <> $4 RETURNING 1)
               SELECT COUNT(*) AS "updated!" FROM moved"#,
            id.as_str(),
            folder.map(DocumentFolderId::as_str),
            paths.from,
            paths.to,
        )
        .fetch_one(&*self.pools.write)
        .await?;
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
        cursor: Option<ListCursor>,
        limit: i64,
    ) -> RepoResult<Vec<DocumentPathChange>> {
        let rows = sqlx::query!(
            r#"SELECT id, document_id::text AS "document_id!: DocumentId",
                      from_path, to_path, changed_at
               FROM document_path_changes
               WHERE ($1::timestamptz IS NULL OR (changed_at, id) > ($1, $2::uuid))
               ORDER BY changed_at, id
               LIMIT $3"#,
            cursor.map(|c| c.updated_at),
            cursor.map(|c| c.id),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DocumentPathChange {
                cursor: ListCursor::new(row.changed_at, row.id),
                document_id: row.document_id,
                from_path: row.from_path,
                to_path: row.to_path,
            })
            .collect())
    }
}
