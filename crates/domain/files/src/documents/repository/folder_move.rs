//! Renaming and moving a folder, each in one tree-locked transaction that
//! also writes the path history of every live document beneath it.
//!
//! A move re-homes the documents other users own in the moving subtree to
//! their owners' roots first, as a recursive trash does, so the folder owner
//! never carries another user's document under a new parent's grants. It
//! then retracts the remaining documents from subjects the former ancestors
//! granted, and refuses when the destination changed after the caller
//! authorized against it: the destination's chain is compared with the one
//! the caller read before authorizing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, UserId};

use super::folder_locks::begin_tree_locked;
use super::folder_paths::{
    ancestors_of, deliver_new_ancestor_grants, foreign_subtree_documents, log_subtree_paths,
    retract_ancestor_grants, subtree_paths,
};
use super::folder_trash::rehome_subtree_documents;
use super::folders::DocumentFolderRepository;
use super::rows::RepoResult;

#[derive(Debug, Clone, Copy)]
pub struct FolderRelocation<'a> {
    pub folder: &'a DocumentFolderId,
    pub parent: Option<&'a DocumentFolderId>,
    pub parent_chain: &'a [DocumentFolderId],
    pub mover: &'a UserId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderMoveOutcome {
    Moved { folders: u64, rehomed: u64 },
    Refused,
    DestinationChanged,
}

impl DocumentFolderRepository {
    pub async fn rename(&self, id: &DocumentFolderId, name: &str) -> RepoResult<bool> {
        let mut tx = begin_tree_locked(&self.pools.write, &[id.as_str()]).await?;
        let before = subtree_paths(&mut tx, id).await?;
        let renamed = sqlx::query!(
            "UPDATE document_folders SET name = $2, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
            name,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if !renamed {
            tx.rollback().await?;
            return Ok(false);
        }
        log_subtree_paths(&mut tx, &before).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn move_to(&self, relocation: FolderRelocation<'_>) -> RepoResult<FolderMoveOutcome> {
        let FolderRelocation {
            folder,
            parent,
            parent_chain,
            mover,
        } = relocation;
        let mut touched = vec![folder.as_str()];
        touched.extend(parent.map(DocumentFolderId::as_str));
        let mut tx = begin_tree_locked(&self.pools.write, &touched).await?;
        if let Some(parent) = parent {
            let chain: Option<Vec<String>> = sqlx::query_scalar!(
                r#"SELECT ancestor_ids::text[] AS "chain!" FROM document_folders
                   WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
                parent.as_str(),
            )
            .fetch_optional(&mut *tx)
            .await?;
            let expected: Vec<&str> = parent_chain.iter().map(DocumentFolderId::as_str).collect();
            if chain.is_none_or(|chain| chain != expected) {
                tx.rollback().await?;
                return Ok(FolderMoveOutcome::DestinationChanged);
            }
        }
        let foreign = foreign_subtree_documents(&mut tx, folder, mover).await?;
        let rehomed = rehome_subtree_documents(&mut tx, folder, mover, &foreign).await?;
        let former_ancestors = ancestors_of(&mut tx, folder).await?;
        retract_ancestor_grants(&mut tx, folder).await?;
        let before = subtree_paths(&mut tx, folder).await?;
        let moved = sqlx::query!(
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
            folder.as_str(),
            parent.map(DocumentFolderId::as_str),
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if moved == 0 {
            tx.rollback().await?;
            return Ok(FolderMoveOutcome::Refused);
        }
        log_subtree_paths(&mut tx, &before).await?;
        deliver_new_ancestor_grants(&mut tx, folder, &former_ancestors).await?;
        tx.commit().await?;
        Ok(FolderMoveOutcome::Moved {
            folders: moved,
            rehomed: u64::try_from(rehomed.max(0)).unwrap_or(0),
        })
    }
}
