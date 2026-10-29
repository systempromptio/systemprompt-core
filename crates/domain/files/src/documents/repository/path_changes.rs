//! Path-changing writes on [`DocumentRepository`] and the path history the
//! bridge Drive change feed reads.
//!
//! Every trash, restore, rename and move writes its `document_path_changes`
//! row in the same statement as the document update, so the history never
//! disagrees with the document row. A move also writes, per subject a grant
//! on the old or new folder chain names, the `lost` or `gained` row the Drive
//! feed uses to retract or deliver the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};

use super::documents::DocumentRepository;
use super::feed::FeedHorizon;
use super::rows::RepoResult;
use crate::documents::models::{DriveCursor, SubjectPairs};

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
    pub access: Option<SubjectAccess>,
}

/// How a path history row that names a grant subject changed that subject's
/// access to the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectAccess {
    Lost,
    Gained,
}

impl SubjectAccess {
    fn parse(value: Option<&str>) -> Option<Self> {
        match value {
            Some("lost") => Some(Self::Lost),
            Some("gained") => Some(Self::Gained),
            _ => None,
        }
    }
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
            r#"WITH before AS (
                   SELECT d.owner_user_id,
                          COALESCE(f.ancestor_ids || f.id, '{}'::uuid[]) AS chain
                   FROM documents d LEFT JOIN document_folders f ON f.id = d.folder_id
                   WHERE d.id = $1::text::uuid),
               after AS (
                   SELECT COALESCE(
                       (SELECT p.ancestor_ids || p.id FROM document_folders p
                        WHERE p.id = $2::text::uuid),
                       '{}'::uuid[]) AS chain),
               moved AS (
                   UPDATE documents SET folder_id = $2::text::uuid, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL
                     AND ($2::text IS NULL OR EXISTS (
                         SELECT 1 FROM document_folders f
                         WHERE f.id = $2::text::uuid AND f.deleted_at IS NULL))
                   RETURNING id),
               logged AS (
                   INSERT INTO document_path_changes (document_id, from_path, to_path)
                   SELECT id, $3, $4 FROM moved WHERE $3 <> $4 RETURNING 1),
               lost AS (
                   INSERT INTO document_path_changes
                       (document_id, from_path, subject_kind, subject_value, subject_access)
                   SELECT DISTINCT m.id, $3, g.subject_kind, g.subject_value, 'lost'
                   FROM moved m, before b, after a, document_grants g
                   WHERE g.folder_id = ANY(b.chain) AND NOT (g.folder_id = ANY(a.chain))
                   RETURNING 1),
               gained AS (
                   INSERT INTO document_path_changes
                       (document_id, to_path, subject_kind, subject_value, subject_access)
                   SELECT DISTINCT m.id, $4, g.subject_kind, g.subject_value, 'gained'
                   FROM moved m, before b, after a, document_grants g
                   WHERE g.folder_id = ANY(a.chain) AND NOT (g.folder_id = ANY(b.chain))
                     AND NOT (g.subject_kind = 'user' AND g.subject_value = b.owner_user_id)
                     AND NOT EXISTS (
                         SELECT 1 FROM document_grants o
                         WHERE o.subject_kind = g.subject_kind
                           AND o.subject_value = g.subject_value
                           AND (o.document_id = m.id OR o.folder_id = ANY(b.chain)))
                   RETURNING 1)
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
        pairs: &SubjectPairs,
        limit: i64,
    ) -> RepoResult<Vec<DocumentPathChange>> {
        let rows = sqlx::query!(
            r#"SELECT document_id::text AS "document_id!: DocumentId",
                      from_path, to_path,
                      subject_access,
                      landing_xid AS "landing_xid!", landing_seq AS "landing_seq!"
               FROM document_path_changes
               WHERE landing_xid IS NOT NULL
                 AND landing_xid < $3
                 AND (landing_xid, landing_seq) > ($1, $2)
                 AND (subject_kind IS NULL
                      OR (subject_kind, subject_value) IN (
                          SELECT k, v FROM UNNEST($5::text[], $6::text[]) AS p(k, v)))
               ORDER BY landing_xid, landing_seq
               LIMIT $4"#,
            after.xid,
            after.seq,
            horizon.0,
            limit.max(1),
            &pairs.kinds,
            &pairs.values,
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
                access: SubjectAccess::parse(row.subject_access.as_deref()),
            })
            .collect())
    }
}
