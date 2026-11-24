//! [`DocumentPublicLinkRepository`]: read-only public links to a document.
//! A link's token and password are stored only as hashes. Creating, opening
//! and revoking a link each write a row to the grant audit log in the same
//! statement. Failed password attempts are counted per link and client
//! address inside a window; the attempts older than it are pruned as new
//! ones are written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, PublicLinkId, UserId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{GrantActor, NewPublicLink, PublicLink, PublicLinkGate};

#[derive(Debug, Clone)]
pub struct DocumentPublicLinkRepository {
    pools: Pools,
}

impl DocumentPublicLinkRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn create(&self, link: &NewPublicLink) -> RepoResult<PublicLink> {
        Ok(sqlx::query_as!(
            PublicLink,
            r#"WITH l AS (
                   INSERT INTO document_public_links
                       (document_id, token_hash, password_hash, expires_at, created_by)
                   VALUES ($1::text::uuid, $2, $3, $4, $5)
                   RETURNING *),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, new_level,
                        valid_until, actor_user_id, trace_id)
                   SELECT 'public_link.created', 'document', l.document_id, 'public_link',
                          l.id::text, 'view', l.expires_at, $5, $6
                   FROM l)
               SELECT id::text AS "id!: PublicLinkId",
                         document_id::text AS "document_id!: DocumentId",
                         password_hash IS NOT NULL AS "has_password!", expires_at, revoked_at,
                         created_by AS "created_by: UserId", created_at, open_count,
                         last_opened_at
               FROM l"#,
            link.document_id as _,
            link.token_hash,
            link.password_hash,
            link.expires_at,
            link.actor.user as _,
            link.actor.trace as _,
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn list(&self, document: &DocumentId) -> RepoResult<Vec<PublicLink>> {
        Ok(sqlx::query_as!(
            PublicLink,
            r#"SELECT id::text AS "id!: PublicLinkId",
                         document_id::text AS "document_id!: DocumentId",
                         password_hash IS NOT NULL AS "has_password!", expires_at, revoked_at,
                         created_by AS "created_by: UserId", created_at, open_count,
                         last_opened_at
               FROM document_public_links
               WHERE document_id = $1::text::uuid
               ORDER BY created_at DESC, id"#,
            document as _,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn revoke(
        &self,
        document: &DocumentId,
        id: &PublicLinkId,
        actor: &GrantActor,
    ) -> RepoResult<bool> {
        let revoked = sqlx::query_scalar!(
            r#"WITH l AS (
                   UPDATE document_public_links SET revoked_at = now()
                   WHERE id = $1::text::uuid AND document_id = $2::text::uuid
                     AND revoked_at IS NULL
                   RETURNING id, document_id),
               e AS (
                   INSERT INTO document_grant_events
                       (event, target_kind, target_id, subject_kind, subject_value, old_level,
                        actor_user_id, trace_id)
                   SELECT 'public_link.revoked', 'document', l.document_id, 'public_link',
                          l.id::text, 'view', $3, $4
                   FROM l)
               SELECT COUNT(*) AS "revoked!" FROM l"#,
            id as _,
            document as _,
            actor.user as _,
            actor.trace as _,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(revoked == 1)
    }

    pub async fn gate(&self, token_hash: &str) -> RepoResult<Option<PublicLinkGate>> {
        Ok(sqlx::query_as!(
            PublicLinkGate,
            r#"SELECT id::text AS "id!: PublicLinkId",
                      document_id::text AS "document_id!: DocumentId",
                      password_hash, expires_at, revoked_at
               FROM document_public_links WHERE token_hash = $1"#,
            token_hash,
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn failed_attempts(
        &self,
        link: &PublicLinkId,
        client_ip: &str,
        since: DateTime<Utc>,
    ) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM document_link_attempts
               WHERE link_id = $1::text::uuid AND client_ip = $2 AND attempted_at > $3"#,
            link as _,
            client_ip,
            since,
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn record_failure(
        &self,
        link: &PublicLinkId,
        client_ip: &str,
        prune_before: DateTime<Utc>,
    ) -> RepoResult<()> {
        sqlx::query!(
            r#"WITH pruned AS (
                   DELETE FROM document_link_attempts
                   WHERE link_id = $1::text::uuid AND attempted_at <= $3)
               INSERT INTO document_link_attempts (link_id, client_ip)
               VALUES ($1::text::uuid, $2)"#,
            link as _,
            client_ip,
            prune_before,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn record_open(&self, link: &PublicLinkId) -> RepoResult<()> {
        sqlx::query!(
            r#"WITH l AS (
                   UPDATE document_public_links
                   SET open_count = open_count + 1, last_opened_at = now()
                   WHERE id = $1::text::uuid
                   RETURNING id, document_id)
               INSERT INTO document_grant_events
                   (event, target_kind, target_id, subject_kind, subject_value, new_level)
               SELECT 'public_link.opened', 'document', l.document_id, 'public_link',
                      l.id::text, 'view'
               FROM l"#,
            link as _,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }
}
