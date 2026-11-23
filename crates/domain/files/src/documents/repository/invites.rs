//! [`DocumentInviteRepository`]: invites to a document addressed to an email
//! outside the instance. The token is stored only as its SHA-256. Creating,
//! revoking and claiming an invite each write a row to the grant audit log in
//! the same statement; a claim is a compare-and-set from open, so an invite is
//! claimed at most once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentInviteId, Email, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentInvite, GrantActor, NewInvite};

#[derive(Debug, Clone)]
pub struct DocumentInviteRepository {
    pools: Pools,
}

struct InviteRow {
    id: DocumentInviteId,
    document_id: DocumentId,
    email: Email,
    access_level: String,
    invited_by: UserId,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    claimed_by_user_id: Option<UserId>,
    claimed_at: Option<DateTime<Utc>>,
}

impl TryFrom<InviteRow> for DocumentInvite {
    type Error = RepositoryError;

    fn try_from(row: InviteRow) -> RepoResult<Self> {
        Ok(Self {
            level: parse("document_invites.access_level", &row.access_level)?,
            id: row.id,
            document_id: row.document_id,
            email: row.email,
            invited_by: row.invited_by,
            created_at: row.created_at,
            expires_at: row.expires_at,
            revoked_at: row.revoked_at,
            claimed_by_user_id: row.claimed_by_user_id,
            claimed_at: row.claimed_at,
        })
    }
}

impl DocumentInviteRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn create(&self, invite: &NewInvite) -> RepoResult<DocumentInvite> {
        sqlx::query_as!(
            InviteRow,
            r#"WITH i AS (
                   INSERT INTO document_invites
                       (document_id, email, access_level, token_hash, invited_by, expires_at)
                   VALUES ($1::text::uuid, $2, $3, $4, $5, $6)
                   RETURNING *),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, new_level,
                        valid_until, actor_user_id, trace_id)
                   SELECT 'invite.created', 'document', i.document_id, 'invite', i.email,
                          i.access_level, i.expires_at, $5, $7
                   FROM i)
               SELECT id::text AS "id!: DocumentInviteId",
                      document_id::text AS "document_id!: DocumentId",
                      email AS "email: Email", access_level, invited_by AS "invited_by: UserId", created_at,
                      expires_at, revoked_at,
                      claimed_by_user_id AS "claimed_by_user_id: UserId", claimed_at
               FROM i"#,
            invite.document_id.as_str(),
            invite.email.as_str(),
            invite.level.as_str(),
            invite.token_hash,
            invite.actor.user.as_str(),
            invite.expires_at,
            invite.actor.trace.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?
        .try_into()
    }

    pub async fn list(&self, document: &DocumentId) -> RepoResult<Vec<DocumentInvite>> {
        sqlx::query_as!(
            InviteRow,
            r#"SELECT id::text AS "id!: DocumentInviteId",
                      document_id::text AS "document_id!: DocumentId",
                      email AS "email: Email", access_level, invited_by AS "invited_by: UserId", created_at,
                      expires_at, revoked_at,
                      claimed_by_user_id AS "claimed_by_user_id: UserId", claimed_at
               FROM document_invites
               WHERE document_id = $1::text::uuid
               ORDER BY created_at DESC, id"#,
            document.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?
        .into_iter()
        .map(DocumentInvite::try_from)
        .collect()
    }

    pub async fn by_token(&self, token_hash: &str) -> RepoResult<Option<DocumentInvite>> {
        sqlx::query_as!(
            InviteRow,
            r#"SELECT id::text AS "id!: DocumentInviteId",
                      document_id::text AS "document_id!: DocumentId",
                      email AS "email: Email", access_level, invited_by AS "invited_by: UserId", created_at,
                      expires_at, revoked_at,
                      claimed_by_user_id AS "claimed_by_user_id: UserId", claimed_at
               FROM document_invites WHERE token_hash = $1"#,
            token_hash,
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentInvite::try_from)
        .transpose()
    }

    pub async fn revoke(
        &self,
        document: &DocumentId,
        id: &DocumentInviteId,
        actor: &GrantActor,
    ) -> RepoResult<bool> {
        let revoked = sqlx::query_scalar!(
            r#"WITH i AS (
                   UPDATE document_invites SET revoked_at = now()
                   WHERE id = $1::text::uuid AND document_id = $2::text::uuid
                     AND revoked_at IS NULL AND claimed_at IS NULL
                   RETURNING id, document_id, email, access_level),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        actor_user_id, trace_id)
                   SELECT 'invite.revoked', 'document', i.document_id, 'invite', i.email,
                          i.access_level, $3, $4
                   FROM i)
               SELECT COUNT(*) AS "revoked!" FROM i"#,
            id.as_str(),
            document.as_str(),
            actor.user.as_str(),
            actor.trace.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(revoked == 1)
    }

    pub async fn mark_claimed(
        &self,
        conn: &mut PgConnection,
        id: &DocumentInviteId,
        actor: &GrantActor,
    ) -> RepoResult<bool> {
        let claimed = sqlx::query_scalar!(
            r#"WITH i AS (
                   UPDATE document_invites
                   SET claimed_by_user_id = $2, claimed_at = now()
                   WHERE id = $1::text::uuid AND claimed_at IS NULL AND revoked_at IS NULL
                     AND expires_at > now()
                   RETURNING id, document_id, email, access_level),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, new_level,
                        actor_user_id, trace_id)
                   SELECT 'invite.claimed', 'document', i.document_id, 'invite', i.email,
                          i.access_level, $2, $3
                   FROM i)
               SELECT COUNT(*) AS "claimed!" FROM i"#,
            id.as_str(),
            actor.user.as_str(),
            actor.trace.as_str(),
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(claimed == 1)
    }
}
