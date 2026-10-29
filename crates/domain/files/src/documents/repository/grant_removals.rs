//! Statements that remove grants: one revoke, every grant on a document,
//! and the expiry sweep. Each writes its `document_grant_events` row and, per
//! live document the grant covered, a path history row naming the subject in
//! the same statement, so the Drive feed can retract the document from a
//! device that lost access.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentId;
use uuid::Uuid;

use super::grants::DocumentGrantRepository;
use super::rows::RepoResult;
use crate::documents::models::GrantActor;

impl DocumentGrantRepository {
    pub async fn delete(&self, id: Uuid, actor: &GrantActor) -> RepoResult<bool> {
        let deleted = sqlx::query_scalar!(
            r#"WITH g AS (DELETE FROM document_grants WHERE id = $1 RETURNING *),
               lost AS (
                   INSERT INTO document_path_changes
                       (document_id, from_path, subject_kind, subject_value, subject_access)
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g JOIN documents d ON d.id = g.document_id
                   WHERE d.deleted_at IS NULL
                   UNION ALL
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g
                   JOIN document_folders f
                     ON (f.id = g.folder_id OR g.folder_id = ANY(f.ancestor_ids))
                    AND f.deleted_at IS NULL
                   JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
                   RETURNING 1),
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

    pub async fn delete_for_document(
        &self,
        id: &DocumentId,
        actor: &GrantActor,
    ) -> RepoResult<u64> {
        let revoked = sqlx::query_scalar!(
            r#"WITH g AS (DELETE FROM document_grants WHERE document_id = $1::text::uuid
                          RETURNING *),
               lost AS (
                   INSERT INTO document_path_changes
                       (document_id, from_path, subject_kind, subject_value, subject_access)
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g JOIN documents d ON d.id = g.document_id
                   WHERE d.deleted_at IS NULL
                   UNION ALL
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g
                   JOIN document_folders f
                     ON (f.id = g.folder_id OR g.folder_id = ANY(f.ancestor_ids))
                    AND f.deleted_at IS NULL
                   JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
                   RETURNING 1),
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
                   WHERE g.subject_kind = 'user' AND p.user_id = g.subject_value
                     AND (p.document_id = g.document_id
                          OR (g.folder_id IS NOT NULL AND EXISTS (
                              SELECT 1 FROM documents d
                              JOIN document_folders f ON f.id = d.folder_id
                              WHERE d.id = p.document_id
                                AND (f.id = g.folder_id OR g.folder_id = ANY(f.ancestor_ids)))))
                   RETURNING 1),
               lost AS (
                   INSERT INTO document_path_changes
                       (document_id, from_path, subject_kind, subject_value, subject_access)
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g JOIN documents d ON d.id = g.document_id
                   WHERE d.deleted_at IS NULL
                   UNION ALL
                   SELECT d.id, document_drive_path(d.folder_id, d.name),
                          g.subject_kind, g.subject_value, 'lost'
                   FROM g
                   JOIN document_folders f
                     ON (f.id = g.folder_id OR g.folder_id = ANY(f.ancestor_ids))
                    AND f.deleted_at IS NULL
                   JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
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
}
