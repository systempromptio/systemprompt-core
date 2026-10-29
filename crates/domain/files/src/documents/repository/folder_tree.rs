//! Statements that reshape a folder tree: create, move and recursive trash.
//!
//! Each runs in one transaction holding the tree lock (`folder_locks`) on the
//! root of every tree it touches, so two reshapes of one tree serialise: the
//! cycle check and the `ancestor_ids` a move rewrites or a create copies are
//! read under the lock and cannot be invalidated before the commit. A subtree
//! trash also locks the subtree's folder rows, so a document insert or move
//! still in flight into one of them settles first, and re-checks after its
//! updates that no live document is left under a trashed folder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_traits::RepositoryError;

use super::documents::DocumentRow;
use super::folder_locks::begin_tree_locked;
use super::folder_trash::{
    rehome_subtree_documents, subtree_has_live_documents, subtree_has_stray,
    trash_subtree_documents,
};
use super::folders::{DocumentFolderRepository, FolderRow};
use super::rows::RepoResult;
use crate::documents::models::{Document, DocumentFolder, NewFolder};

#[derive(Debug, Clone, Copy)]
pub struct SubtreeTrashPlan<'a> {
    pub folders: &'a [DocumentFolderId],
    pub trash: &'a [DocumentId],
    pub rehome: &'a [DocumentId],
}

/// The outcome of trashing a folder subtree. `Changed` means a live folder or
/// document the caller had not partitioned sat in the subtree when the lock
/// was taken; nothing was trashed or re-homed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtreeTrash {
    Trashed {
        documents: u64,
        rehomed: u64,
        folders: u64,
    },
    Changed,
}

impl DocumentFolderRepository {
    pub async fn insert(&self, request: NewFolder) -> RepoResult<DocumentFolder> {
        let parent: Vec<&str> = request
            .parent_id
            .iter()
            .map(DocumentFolderId::as_str)
            .collect();
        let mut tx = begin_tree_locked(&self.pools.write, &parent).await?;
        let row = sqlx::query_as!(
            FolderRow,
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
                         created_at, updated_at, deleted_at, agent_edit_policy"#,
            request.parent_id.as_ref().map(DocumentFolderId::as_str),
            request.name,
            request.owner_user_id.as_str(),
            request.created_by.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let row = row.ok_or_else(|| {
            RepositoryError::not_found(
                "document folder",
                request
                    .parent_id
                    .as_ref()
                    .map_or("<root>", DocumentFolderId::as_str),
            )
        })?;
        tx.commit().await?;
        row.try_into()
    }

    pub async fn move_to(
        &self,
        id: &DocumentFolderId,
        new_parent: Option<&DocumentFolderId>,
    ) -> RepoResult<u64> {
        let mut touched = vec![id.as_str()];
        touched.extend(new_parent.map(DocumentFolderId::as_str));
        let mut tx = begin_tree_locked(&self.pools.write, &touched).await?;
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
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn subtree_folder_ids(
        &self,
        root: &DocumentFolderId,
    ) -> RepoResult<Vec<DocumentFolderId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id::text AS "id!: DocumentFolderId"
               FROM document_folders
               WHERE (id = $1::text::uuid OR $1::text::uuid = ANY(ancestor_ids))
                 AND deleted_at IS NULL
               ORDER BY cardinality(ancestor_ids), id"#,
            root.as_str(),
        )
        .fetch_all(self.pools.authz())
        .await?)
    }

    pub async fn subtree_documents(
        &self,
        root: &DocumentFolderId,
        after: Option<&DocumentId>,
        limit: i64,
    ) -> RepoResult<Vec<Document>> {
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
               FROM documents d
               JOIN document_folders f ON f.id = d.folder_id
               WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND d.deleted_at IS NULL
                 AND ($2::text IS NULL OR d.id > $2::text::uuid)
               ORDER BY d.id LIMIT $3"#,
            root.as_str(),
            after.map(DocumentId::as_str),
            limit.max(1),
        )
        .fetch_all(self.pools.authz())
        .await?
        .into_iter()
        .map(Document::try_from)
        .collect()
    }

    pub async fn trash_subtree(
        &self,
        root: &DocumentFolderId,
        by: &UserId,
        plan: SubtreeTrashPlan<'_>,
    ) -> RepoResult<SubtreeTrash> {
        let folders: Vec<String> = plan
            .folders
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let authorized: Vec<String> = plan.trash.iter().map(|id| id.as_str().to_owned()).collect();
        let rehome: Vec<String> = plan
            .rehome
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let partitioned: Vec<String> = authorized.iter().chain(&rehome).cloned().collect();
        let mut tx = begin_tree_locked(&self.pools.write, &[root.as_str()]).await?;
        sqlx::query_scalar!(
            r#"SELECT id::text AS "id!" FROM document_folders
               WHERE (id = $1::text::uuid OR $1::text::uuid = ANY(ancestor_ids))
                 AND deleted_at IS NULL
               ORDER BY id FOR UPDATE"#,
            root.as_str(),
        )
        .fetch_all(&mut *tx)
        .await?;
        let stray = subtree_has_stray(&mut tx, root, &partitioned, &folders).await?;
        if stray {
            tx.rollback().await?;
            return Ok(SubtreeTrash::Changed);
        }
        let documents = trash_subtree_documents(&mut tx, root, by, &authorized).await?;
        let rehomed = rehome_subtree_documents(&mut tx, root, &rehome).await?;
        let trashed_folders = sqlx::query!(
            "UPDATE document_folders SET deleted_at = now(), updated_at = now()
             WHERE (id = $1::text::uuid OR $1::text::uuid = ANY(ancestor_ids))
               AND deleted_at IS NULL",
            root.as_str(),
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let remaining = subtree_has_live_documents(&mut tx, root).await?;
        if remaining {
            tx.rollback().await?;
            return Ok(SubtreeTrash::Changed);
        }
        tx.commit().await?;
        Ok(SubtreeTrash::Trashed {
            documents: u64::try_from(documents.max(0)).unwrap_or(0),
            rehomed: u64::try_from(rehomed.max(0)).unwrap_or(0),
            folders: trashed_folders,
        })
    }
}
