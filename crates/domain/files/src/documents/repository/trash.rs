//! The document trash and ownership moves: soft delete with the trashing
//! user, restore, the retention sweep's hard delete, and owner transfer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};

use super::documents::{DocumentRepository, DocumentRow};
use super::rows::RepoResult;
use crate::documents::models::Document;

impl DocumentRepository {
    pub async fn find_including_trashed(&self, id: &DocumentId) -> RepoResult<Option<Document>> {
        sqlx::query_as!(
            DocumentRow,
            r#"SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId"
               FROM documents d WHERE d.id = $1::text::uuid"#,
            id.as_str()
        )
        .fetch_optional(self.pools.authz())
        .await?
        .map(Document::try_from)
        .transpose()
    }

    pub async fn purge_trashed_before(&self, cutoff: DateTime<Utc>, limit: i64) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM documents WHERE id IN (
                 SELECT id FROM documents WHERE deleted_at IS NOT NULL AND deleted_at < $1
                 ORDER BY deleted_at LIMIT $2)",
            cutoff,
            limit,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn hard_delete(&self, id: &DocumentId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM documents WHERE id = $1::text::uuid",
            id.as_str()
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn rename_any(&self, id: &DocumentId, name: &str) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET name = $2, updated_at = now() WHERE id = $1::text::uuid",
            id.as_str(),
            name,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn move_to_any(
        &self,
        id: &DocumentId,
        folder: Option<&DocumentFolderId>,
    ) -> RepoResult<bool> {
        let folders: Vec<&str> = folder.map(DocumentFolderId::as_str).into_iter().collect();
        let mut tx = super::folder_locks::begin_tree_locked(&self.pools.write, &folders).await?;
        let result = sqlx::query!(
            "UPDATE documents SET folder_id = $2::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid
               AND ($2::text IS NULL OR EXISTS (
                   SELECT 1 FROM document_folders f
                   WHERE f.id = $2::text::uuid AND f.deleted_at IS NULL))",
            id.as_str(),
            folder.map(DocumentFolderId::as_str),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }
}
