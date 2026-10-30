//! The engine peer ids the server assigned to each join of an edit
//! session, and the conditional append that logs an update only on top of
//! the log its validation saw.
//!
//! A peer id is drawn by the server and recorded against the joining user,
//! so an update's operations can be bound to peers its author holds. Peer
//! ids are positive and below 2^53, so they survive a JSON number.
//!
//! [`DocumentCollabRepository::append_on`] takes the session row lock and
//! refuses the append unless the session's next sequence number is the one
//! after what the caller validated against, so validation and logging see
//! one log whatever a snapshot has pruned. Every logged update records the
//! actor it ran as and the access level it was admitted at, which
//! [`DocumentCollabRepository::list_authors_since_checkpoint`] reads back for
//! the updates the session's last checkpoint does not cover.
//!
//! [`DocumentCollabRepository::refresh_editing`] checks an editor's web
//! presence and extends it in one statement, so every accepted update keeps
//! its author present. [`DocumentCollabRepository::upsert_awareness`] keeps
//! an editing presence editing whatever mode the awareness state names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AgentName, DocumentEditSessionId, DocumentId, UserId};

use super::DocumentCollabRepository;
use super::rows::{RepoResult, presence_source_str, presence_state_str};
use crate::documents::agents::DocumentActor;
use crate::documents::model::EditLevel;
use crate::documents::models::PresenceUpsert;

#[derive(Debug, Clone, Copy)]
pub struct UpdateAuthor<'a> {
    pub user: &'a UserId,
    pub actor: &'a DocumentActor,
    pub level: EditLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedAuthor {
    pub seq: i64,
    pub user: UserId,
    pub actor: Option<DocumentActor>,
    pub level: Option<EditLevel>,
}

pub(super) struct ActorColumns<'a> {
    pub(super) kind: &'static str,
    pub(super) name: Option<&'a str>,
    pub(super) attended: Option<bool>,
}

impl<'a> ActorColumns<'a> {
    pub(super) fn of(actor: &'a DocumentActor) -> Self {
        match actor {
            DocumentActor::Human => Self {
                kind: "human",
                name: None,
                attended: None,
            },
            DocumentActor::Agent(agent) => Self {
                kind: "agent",
                name: Some(agent.name.as_str()),
                attended: Some(agent.attended),
            },
            DocumentActor::System { name } => Self {
                kind: "system",
                name: Some(name),
                attended: None,
            },
        }
    }
}

pub(super) fn actor_from(
    kind: Option<&str>,
    name: Option<String>,
    attended: Option<bool>,
) -> Option<DocumentActor> {
    match (kind?, name) {
        ("human", _) => Some(DocumentActor::Human),
        ("agent", Some(name)) => Some(DocumentActor::agent(
            AgentName::new(name),
            attended.unwrap_or(false),
        )),
        ("system", Some(name)) => Some(DocumentActor::system(name)),
        _ => None,
    }
}

pub(super) const fn level_str(level: EditLevel) -> &'static str {
    match level {
        EditLevel::Edit => "edit",
        EditLevel::Comment => "comment",
    }
}

pub(super) fn level_from(level: &str) -> Option<EditLevel> {
    match level {
        "edit" => Some(EditLevel::Edit),
        "comment" => Some(EditLevel::Comment),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabAppend {
    Appended(i64),
    Behind,
    Closed,
}

impl DocumentCollabRepository {
    pub async fn assign_peer(
        &self,
        session: &DocumentEditSessionId,
        user: &UserId,
        peer: i64,
    ) -> RepoResult<()> {
        sqlx::query!(
            "INSERT INTO document_collab_peers (session_id, peer_id, user_id)
             VALUES ($1::text::uuid, $2, $3)",
            session.as_str(),
            peer,
            user.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn list_peers(
        &self,
        session: &DocumentEditSessionId,
        user: &UserId,
    ) -> RepoResult<Vec<i64>> {
        Ok(sqlx::query_scalar!(
            "SELECT peer_id FROM document_collab_peers
             WHERE session_id = $1::text::uuid AND user_id = $2",
            session.as_str(),
            user.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn head_seq(&self, session: &DocumentEditSessionId) -> RepoResult<Option<i64>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT next_seq - 1 AS "seq!" FROM document_edit_sessions
               WHERE id = $1::text::uuid AND closed_at IS NULL"#,
            session.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn append_on(
        &self,
        session: &DocumentEditSessionId,
        validated_through: i64,
        update: &[u8],
        author: UpdateAuthor<'_>,
    ) -> RepoResult<CollabAppend> {
        let mut tx = self.pools.write.begin().await?;
        let Some(seq) = sqlx::query_scalar!(
            "SELECT next_seq FROM document_edit_sessions
             WHERE id = $1::text::uuid AND closed_at IS NULL
             FOR UPDATE",
            session.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(CollabAppend::Closed);
        };
        if seq != validated_through.saturating_add(1) {
            return Ok(CollabAppend::Behind);
        }
        sqlx::query!(
            "UPDATE document_edit_sessions SET next_seq = next_seq + 1, updated_at = now()
             WHERE id = $1::text::uuid",
            session.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        let columns = ActorColumns::of(author.actor);
        sqlx::query!(
            "INSERT INTO document_collab_updates
                 (session_id, seq, payload, author_user_id, actor_kind, actor_name,
                  actor_attended, edit_level)
             VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7, $8)",
            session.as_str(),
            seq,
            update,
            author.user.as_str(),
            columns.kind,
            columns.name,
            columns.attended,
            level_str(author.level),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(CollabAppend::Appended(seq))
    }

    pub async fn list_authors_since_checkpoint(
        &self,
        session: &DocumentEditSessionId,
    ) -> RepoResult<Vec<LoggedAuthor>> {
        let rows = sqlx::query!(
            r#"SELECT u.seq, u.author_user_id AS "user: UserId", u.actor_kind, u.actor_name,
                      u.actor_attended, u.edit_level
               FROM document_collab_updates u
               JOIN document_edit_sessions s ON s.id = u.session_id
               WHERE u.session_id = $1::text::uuid AND u.seq > s.checkpoint_seq
               ORDER BY u.seq"#,
            session.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| LoggedAuthor {
                seq: row.seq,
                user: row.user,
                actor: actor_from(
                    row.actor_kind.as_deref(),
                    row.actor_name,
                    row.actor_attended,
                ),
                level: row.edit_level.as_deref().and_then(level_from),
            })
            .collect())
    }

    pub async fn refresh_editing(
        &self,
        document_id: &DocumentId,
        user: &UserId,
        ttl: Duration,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_presence
             SET expires_at = now() + $3::float8 * interval '1 second', updated_at = now()
             WHERE document_id = $1::text::uuid AND user_id = $2 AND source = 'web'
               AND state = 'editing' AND expires_at > now()",
            document_id.as_str(),
            user.as_str(),
            ttl.as_secs_f64(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn upsert_awareness(
        &self,
        presence: &PresenceUpsert,
        ttl: Duration,
    ) -> RepoResult<()> {
        sqlx::query!(
            "INSERT INTO document_presence (document_id, user_id, source, state, cursor, expires_at)
             VALUES ($1::text::uuid, $2, $3, $4, $5, now() + $6::float8 * interval '1 second')
             ON CONFLICT (document_id, user_id, source) DO UPDATE
                 SET state = CASE
                         WHEN document_presence.state = 'editing'
                              AND document_presence.expires_at > now() THEN 'editing'
                         ELSE EXCLUDED.state
                     END,
                     cursor = EXCLUDED.cursor,
                     updated_at = now(),
                     expires_at = EXCLUDED.expires_at",
            presence.document_id.as_str(),
            presence.user_id.as_str(),
            presence_source_str(presence.source),
            presence_state_str(presence.state),
            presence.cursor,
            ttl.as_secs_f64(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }
}
