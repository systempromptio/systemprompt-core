//! The child folders of one parent the caller can see, read a page at a time.
//!
//! One statement per page decides visibility for every child at once: the
//! caller owns the child, or one of the caller's subject pairs holds an
//! unexpired grant on the child or any folder in its ancestor chain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, UserId};

use super::folders::{DocumentFolderRepository, FolderRow};
use super::rows::RepoResult;
use crate::documents::models::{DocumentFolder, SubjectPairs};

/// One page request over a parent's visible children, keyed by `(name, id)`.
#[derive(Debug, Clone, Copy)]
pub struct ChildFolderPage<'a> {
    pub parent: Option<&'a DocumentFolderId>,
    pub caller: &'a UserId,
    pub subjects: &'a SubjectPairs,
    pub pattern: Option<&'a str>,
    pub after: Option<&'a DocumentFolder>,
    pub limit: i64,
}

impl DocumentFolderRepository {
    pub async fn list_visible_children(
        &self,
        page: ChildFolderPage<'_>,
    ) -> RepoResult<Vec<DocumentFolder>> {
        sqlx::query_as!(
            FolderRow,
            r#"WITH subject AS (
                   SELECT s.kind, s.value FROM UNNEST($3::text[], $4::text[]) AS s(kind, value)
               )
               SELECT f.id::text AS "id!: DocumentFolderId",
                      f.parent_id::text AS "parent_id: DocumentFolderId",
                      f.name,
                      f.ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      f.owner_user_id AS "owner_user_id: UserId",
                      f.created_by AS "created_by: UserId",
                      f.created_at, f.updated_at, f.deleted_at, f.agent_edit_policy
               FROM document_folders f
               WHERE f.parent_id IS NOT DISTINCT FROM $1::text::uuid
                 AND f.deleted_at IS NULL
                 AND ($5::text IS NULL OR f.name ILIKE $5)
                 AND ($6::text IS NULL OR (f.name, f.id) > ($6::text, $7::text::uuid))
                 AND (f.owner_user_id = $2 OR EXISTS (
                     SELECT 1 FROM document_grants g
                     JOIN subject s ON s.kind = g.subject_kind AND s.value = g.subject_value
                     WHERE g.folder_id = ANY(f.ancestor_ids || f.id)
                       AND (g.valid_until IS NULL OR g.valid_until > now())))
               ORDER BY f.name, f.id
               LIMIT $8"#,
            page.parent.map(DocumentFolderId::as_str),
            page.caller.as_str(),
            &page.subjects.kinds,
            &page.subjects.values,
            page.pattern,
            page.after.map(|folder| folder.name.as_str()),
            page.after.map(|folder| folder.id.as_str()),
            page.limit,
        )
        .fetch_all(self.pools.authz())
        .await?
        .into_iter()
        .map(DocumentFolder::try_from)
        .collect()
    }
}
