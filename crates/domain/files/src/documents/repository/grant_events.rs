//! [`DocumentGrantEventRepository`]: the grant audit log, and ownership
//! transfer, which moves the owner, keeps the previous owner an `edit` grant
//! and records both in one statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId, UserId};
use systemprompt_traits::RepositoryError;
use uuid::Uuid;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentGrantEvent, GrantTarget, OwnerTransfer};

#[derive(Debug, Clone)]
pub struct DocumentGrantEventRepository {
    pools: Pools,
}

struct EventRow {
    id: Uuid,
    event: String,
    document_id: Option<DocumentId>,
    folder_id: Option<DocumentFolderId>,
    subject_kind: String,
    subject_value: String,
    old_level: Option<String>,
    new_level: Option<String>,
    valid_until: Option<DateTime<Utc>>,
    actor_user_id: Option<UserId>,
    trace_id: Option<TraceId>,
    created_at: DateTime<Utc>,
}

impl TryFrom<EventRow> for DocumentGrantEvent {
    type Error = RepositoryError;

    fn try_from(row: EventRow) -> RepoResult<Self> {
        let target = match (row.document_id, row.folder_id) {
            (Some(document), None) => GrantTarget::Document(document),
            (None, Some(folder)) => GrantTarget::Folder(folder),
            _ => {
                return Err(RepositoryError::invalid_data(
                    "document_grant_events.target_kind",
                    "target_kind must be document or folder",
                ));
            },
        };
        Ok(Self {
            id: row.id,
            event: parse("document_grant_events.event", &row.event)?,
            target,
            subject_kind: row.subject_kind,
            subject_value: row.subject_value,
            old_level: row
                .old_level
                .map(|level| parse("document_grant_events.old_level", &level))
                .transpose()?,
            new_level: row
                .new_level
                .map(|level| parse("document_grant_events.new_level", &level))
                .transpose()?,
            valid_until: row.valid_until,
            actor_user_id: row.actor_user_id,
            trace_id: row.trace_id,
            created_at: row.created_at,
        })
    }
}

impl DocumentGrantEventRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn list_for_target(
        &self,
        target: &GrantTarget,
        limit: i64,
    ) -> RepoResult<Vec<DocumentGrantEvent>> {
        let (kind, id) = match target {
            GrantTarget::Document(id) => ("document", id.as_str()),
            GrantTarget::Folder(id) => ("folder", id.as_str()),
        };
        sqlx::query_as!(
            EventRow,
            r#"SELECT id, event,
                      CASE WHEN target_kind = 'document' THEN target_id::text END
                          AS "document_id: DocumentId",
                      CASE WHEN target_kind = 'folder' THEN target_id::text END
                          AS "folder_id: DocumentFolderId",
                      subject_kind,
                      subject_value, old_level, new_level, valid_until,
                      actor_user_id AS "actor_user_id: UserId",
                      trace_id AS "trace_id: TraceId", created_at
               FROM document_grant_events
               WHERE target_id = $1::text::uuid AND target_kind = $2
               ORDER BY created_at DESC, id DESC LIMIT $3"#,
            id,
            kind,
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrantEvent::try_from)
        .collect()
    }

    pub async fn transfer_owner(&self, transfer: &OwnerTransfer) -> RepoResult<bool> {
        let moved = sqlx::query_scalar!(
            r#"WITH d AS (
                   UPDATE documents SET owner_user_id = $3, updated_at = now()
                   WHERE id = $1::text::uuid AND deleted_at IS NULL AND owner_user_id = $2
                   RETURNING id),
               prev AS (
                   SELECT g.access_level FROM document_grants g, d
                   WHERE g.document_id = d.id AND g.subject_kind = 'user'
                     AND g.subject_value = $2),
               g AS (
                   INSERT INTO document_grants
                       (document_id, subject_kind, subject_value, subject_display,
                        access_level, granted_by)
                   SELECT d.id, 'user', $2, $4, 'edit', $5 FROM d WHERE $2 <> $3
                   ON CONFLICT (document_id, folder_id, subject_kind, subject_value) DO UPDATE
                       SET access_level = 'edit', valid_until = NULL,
                           granted_by = EXCLUDED.granted_by, updated_at = now()
                   RETURNING document_id),
               moved AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        new_level, actor_user_id, trace_id)
                   SELECT 'transferred', 'document', d.id, 'user', $3, 'own', 'own', $5, $6
                   FROM d),
               kept AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        new_level, actor_user_id, trace_id)
                   SELECT CASE WHEN EXISTS (SELECT 1 FROM prev) THEN 'changed' ELSE 'granted' END,
                          'document', g.document_id, 'user', $2,
                          (SELECT access_level FROM prev), 'edit', $5, $6
                   FROM g)
               SELECT COUNT(*) AS "moved!" FROM d"#,
            transfer.document_id.as_str(),
            transfer.from.as_str(),
            transfer.to.as_str(),
            transfer.from_display,
            transfer.actor.user.as_str(),
            transfer.actor.trace.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(moved == 1)
    }
}
