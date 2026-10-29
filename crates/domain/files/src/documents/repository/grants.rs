//! [`DocumentGrantRepository`]: subject-keyed grants on documents and
//! folders, and the effective level a set of subject pairs holds. Every
//! statement that changes a grant writes its `document_grant_events` row in
//! the same statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};
use systemprompt_models::documents::AccessLevel;
use uuid::Uuid;

use super::Pools;
use super::grant_row::{GrantRow, level_from_rank};
use super::rows::RepoResult;
use crate::documents::models::{DocumentGrant, GrantActor, GrantTarget, NewGrant, SubjectPairs};

#[derive(Debug, Clone)]
pub struct DocumentGrantRepository {
    pub(super) pools: Pools,
}

impl DocumentGrantRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewGrant) -> RepoResult<DocumentGrant> {
        let (document_id, folder_id) = match &request.target {
            GrantTarget::Document(id) => (Some(id.as_str()), None),
            GrantTarget::Folder(id) => (None, Some(id.as_str())),
        };
        let row = sqlx::query_as!(
            GrantRow,
            r#"WITH prev AS (
                   SELECT access_level FROM document_grants
                   WHERE document_id IS NOT DISTINCT FROM $1::text::uuid
                     AND folder_id IS NOT DISTINCT FROM $2::text::uuid
                     AND subject_kind = $3 AND subject_value = $4),
               g AS (INSERT INTO document_grants
                   (document_id, folder_id, subject_kind, subject_value, subject_display,
                    access_level, granted_by, valid_until)
               VALUES ($1::text::uuid, $2::text::uuid, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (document_id, folder_id, subject_kind, subject_value) DO UPDATE
                   SET access_level = EXCLUDED.access_level,
                       valid_until = EXCLUDED.valid_until,
                       granted_by = EXCLUDED.granted_by,
                       subject_display = COALESCE(EXCLUDED.subject_display,
                                                  document_grants.subject_display),
                       updated_at = now()
               RETURNING *),
               e AS (INSERT INTO document_grant_events
                   (event, target_kind, target_id, subject_kind, subject_value, old_level,
                    new_level, valid_until, actor_user_id, trace_id, reason, via_agent)
               SELECT CASE WHEN EXISTS (SELECT 1 FROM prev) THEN 'changed' ELSE 'granted' END,
                      CASE WHEN g.document_id IS NULL THEN 'folder' ELSE 'document' END,
                      COALESCE(g.document_id, g.folder_id), g.subject_kind, g.subject_value,
                      (SELECT access_level FROM prev), g.access_level, g.valid_until,
                      $7, $9, $10, $11
               FROM g)
               SELECT g.id AS "id!", g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind AS "subject_kind!", g.subject_value AS "subject_value!",
                      g.access_level AS "access_level!",
                      g.granted_by AS "granted_by!: UserId",
                      g.valid_until, g.created_at AS "created_at!", g.updated_at AS "updated_at!",
                      g.subject_display
               FROM g"#,
            document_id,
            folder_id,
            request.subject_kind,
            request.subject_value,
            request.subject_display,
            request.access_level.as_str(),
            request.granted_by.as_str(),
            request.valid_until,
            request.trace_id.as_str(),
            request.reason,
            request.via_agent,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        row.try_into()
    }

    pub async fn delete(&self, id: Uuid, actor: &GrantActor) -> RepoResult<bool> {
        let deleted = sqlx::query_scalar!(
            r#"WITH g AS (DELETE FROM document_grants WHERE id = $1 RETURNING *),
               e AS (INSERT INTO document_grant_events
                   (event, target_kind, target_id, subject_kind, subject_value, old_level,
                    valid_until, actor_user_id, trace_id)
               SELECT 'revoked', CASE WHEN g.document_id IS NULL THEN 'folder' ELSE 'document' END,
                      COALESCE(g.document_id, g.folder_id), g.subject_kind, g.subject_value,
                      g.access_level, g.valid_until, $2, $3
               FROM g)
               SELECT COUNT(*) AS "deleted!" FROM g"#,
            id,
            actor.user.as_str(),
            actor.trace.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(deleted == 1)
    }

    pub async fn list_for_document(&self, id: &DocumentId) -> RepoResult<Vec<DocumentGrant>> {
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.document_id = $1::text::uuid ORDER BY g.created_at, g.id"#,
            id.as_str()
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn list_for_folder(&self, id: &DocumentFolderId) -> RepoResult<Vec<DocumentGrant>> {
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.folder_id = $1::text::uuid ORDER BY g.created_at, g.id"#,
            id.as_str()
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn list_for_folders(
        &self,
        folders: &[DocumentFolderId],
    ) -> RepoResult<Vec<DocumentGrant>> {
        let folders: Vec<String> = folders.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.folder_id = ANY($1::text[]::uuid[]) ORDER BY g.created_at, g.id"#,
            &folders
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn delete_for_document(
        &self,
        id: &DocumentId,
        actor: &GrantActor,
    ) -> RepoResult<u64> {
        let revoked = sqlx::query_scalar!(
            r#"WITH g AS (DELETE FROM document_grants WHERE document_id = $1::text::uuid
                          RETURNING *),
               e AS (INSERT INTO document_grant_events
                   (event, target_kind, target_id, subject_kind, subject_value, old_level,
                    valid_until, actor_user_id, trace_id)
               SELECT 'revoked_all', 'document', g.document_id, g.subject_kind,
                      g.subject_value, g.access_level, g.valid_until, $2, $3
               FROM g)
               SELECT COUNT(*) AS "revoked!" FROM g"#,
            id.as_str(),
            actor.user.as_str(),
            actor.trace.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(u64::try_from(revoked).unwrap_or(0))
    }

    pub async fn sweep_expired(&self, limit: i64) -> RepoResult<u64> {
        let swept = sqlx::query_scalar!(
            r#"WITH g AS (
                   DELETE FROM document_grants
                   WHERE id IN (SELECT id FROM document_grants
                                WHERE valid_until <= now() LIMIT $1)
                   RETURNING *),
               p AS (
                   DELETE FROM document_presence p USING g
                   WHERE g.subject_kind = 'user' AND p.document_id = g.document_id
                     AND p.user_id = g.subject_value
                   RETURNING 1),
               e AS (INSERT INTO document_grant_events
                   (event, target_kind, target_id, subject_kind, subject_value, old_level,
                    valid_until)
               SELECT 'expired', CASE WHEN g.document_id IS NULL THEN 'folder' ELSE 'document' END,
                      COALESCE(g.document_id, g.folder_id), g.subject_kind, g.subject_value,
                      g.access_level, g.valid_until
               FROM g)
               SELECT COUNT(*) AS "swept!" FROM g"#,
            limit.max(1),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(u64::try_from(swept.max(0)).unwrap_or(0))
    }

    pub async fn effective_level(
        &self,
        document_id: &DocumentId,
        folder_ancestors: &[DocumentFolderId],
        subjects: &SubjectPairs,
    ) -> RepoResult<Option<AccessLevel>> {
        let folders: Vec<String> = folder_ancestors
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let rank = sqlx::query_scalar!(
            r#"SELECT MAX(CASE access_level
                          WHEN 'view' THEN 1 WHEN 'comment' THEN 2 WHEN 'edit' THEN 3
                          WHEN 'share' THEN 4 WHEN 'own' THEN 5 END) AS "rank"
               FROM document_grants g
               WHERE (g.document_id = $1::text::uuid OR g.folder_id = ANY($2::text[]::uuid[]))
                 AND (g.subject_kind, g.subject_value) IN (
                     SELECT * FROM UNNEST($3::text[], $4::text[]))
                 AND (g.valid_until IS NULL OR g.valid_until > now())"#,
            document_id.as_str(),
            &folders,
            &subjects.kinds,
            &subjects.values,
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok(rank.and_then(level_from_rank))
    }
}
