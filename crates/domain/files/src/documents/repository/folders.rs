//! [`DocumentFolderRepository`]: the folder tree with materialised ancestors.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, UserId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{DocumentFolder, NewFolder};

#[derive(Debug, Clone)]
pub struct DocumentFolderRepository {
    pools: Pools,
}

impl DocumentFolderRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewFolder) -> RepoResult<DocumentFolder> {
        let row = sqlx::query_as!(
            DocumentFolder,
            r#"INSERT INTO document_folders (parent_id, name, ancestor_ids, owner_user_id, created_by)
               SELECT $1::text::uuid, $2,
                      COALESCE((SELECT p.ancestor_ids || p.id FROM document_folders p
                                WHERE p.id = $1::text::uuid AND p.deleted_at IS NULL),
                               '{}'::uuid[]),
                      $3, $4
               WHERE $1::text IS NULL OR EXISTS (
                   SELECT 1 FROM document_folders p
                   WHERE p.id = $1::text::uuid AND p.deleted_at IS NULL)
               RETURNING id::text AS "id!: DocumentFolderId",
                         parent_id::text AS "parent_id: DocumentFolderId",
                         name,
                         ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                         owner_user_id AS "owner_user_id: UserId",
                         created_by AS "created_by: UserId",
                         created_at, updated_at, deleted_at"#,
            request.parent_id.as_ref().map(DocumentFolderId::as_str),
            request.name,
            request.owner_user_id.as_str(),
            request.created_by.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?;
        row.ok_or_else(|| {
            systemprompt_traits::RepositoryError::not_found(
                "document folder",
                request
                    .parent_id
                    .as_ref()
                    .map_or("<root>", DocumentFolderId::as_str),
            )
        })
    }

    pub async fn find(&self, id: &DocumentFolderId) -> RepoResult<Option<DocumentFolder>> {
        Ok(sqlx::query_as!(
            DocumentFolder,
            r#"SELECT id::text AS "id!: DocumentFolderId",
                      parent_id::text AS "parent_id: DocumentFolderId",
                      name,
                      ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      owner_user_id AS "owner_user_id: UserId",
                      created_by AS "created_by: UserId",
                      created_at, updated_at, deleted_at
               FROM document_folders WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
            id.as_str()
        )
        .fetch_optional(&*self.pools.read)
        .await?)
    }

    pub async fn list_children(
        &self,
        parent: Option<&DocumentFolderId>,
    ) -> RepoResult<Vec<DocumentFolder>> {
        Ok(sqlx::query_as!(
DocumentFolder,
r#"SELECT id::text AS "id!: DocumentFolderId",
                      parent_id::text AS "parent_id: DocumentFolderId",
                      name,
                      ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      owner_user_id AS "owner_user_id: UserId",
                      created_by AS "created_by: UserId",
                      created_at, updated_at, deleted_at
               FROM document_folders WHERE parent_id IS NOT DISTINCT FROM $1::text::uuid AND deleted_at IS NULL ORDER BY name, id"#,
            parent.map(DocumentFolderId::as_str)
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }

    pub async fn rename(&self, id: &DocumentFolderId, name: &str) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_folders SET name = $2, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
            name,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn move_to(
        &self,
        id: &DocumentFolderId,
        new_parent: Option<&DocumentFolderId>,
    ) -> RepoResult<u64> {
        let result = sqlx::query!(
            r#"WITH target AS (
                   SELECT id, ancestor_ids FROM document_folders
                   WHERE id = $1::text::uuid AND deleted_at IS NULL
               ),
               destination AS (
                   SELECT COALESCE(
                       (SELECT p.ancestor_ids || p.id FROM document_folders p
                        WHERE p.id = $2::text::uuid AND p.deleted_at IS NULL),
                       '{}'::uuid[]) AS ancestors
               )
               UPDATE document_folders f
               SET ancestor_ids = d.ancestors || f.ancestor_ids[cardinality(t.ancestor_ids) + 1:],
                   parent_id = CASE WHEN f.id = t.id THEN $2::text::uuid ELSE f.parent_id END,
                   updated_at = now()
               FROM target t, destination d
               WHERE (f.id = t.id OR t.id = ANY(f.ancestor_ids))
                 AND ($2::text IS NULL OR EXISTS (
                     SELECT 1 FROM document_folders p
                     WHERE p.id = $2::text::uuid AND p.deleted_at IS NULL
                       AND p.id <> t.id AND NOT (t.id = ANY(p.ancestor_ids))))"#,
            id.as_str(),
            new_parent.map(DocumentFolderId::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn soft_delete(&self, id: &DocumentFolderId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_folders SET deleted_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn ancestors(&self, id: &DocumentFolderId) -> RepoResult<Vec<DocumentFolderId>> {
        let row = sqlx::query_scalar!(
            r#"SELECT ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>"
               FROM document_folders WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(&*self.pools.read)
        .await?;
        row.ok_or_else(|| systemprompt_traits::RepositoryError::not_found("document folder", id))
    }

    pub async fn path(&self, id: &DocumentFolderId) -> RepoResult<String> {
        let path = sqlx::query_scalar!(
            r#"SELECT string_agg(a.name, '/' ORDER BY u.ord) AS "path"
               FROM document_folders f
               CROSS JOIN LATERAL unnest(f.ancestor_ids || f.id) WITH ORDINALITY AS u(folder_id, ord)
               JOIN document_folders a ON a.id = u.folder_id
               WHERE f.id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        path.ok_or_else(|| systemprompt_traits::RepositoryError::not_found("document folder", id))
    }
}
