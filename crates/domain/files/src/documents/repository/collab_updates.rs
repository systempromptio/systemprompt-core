//! [`DocumentCollabRepository`]: the CRDT update log of an edit session, the
//! sequence number its stored snapshot covers, and the web presence rows the
//! co-editing service joins and leaves.
//!
//! An update's sequence number is drawn from its session row under a row
//! lock in the transaction that logs it, so sequence order is commit order
//! and a reader that has seen `seq` has seen every update below it.
//!
//! A snapshot only moves forward: [`DocumentCollabRepository::set_snapshot`]
//! records a snapshot covering `seq`, with the version vector it encodes,
//! only when the stored one covers less, and prunes the updates the new
//! snapshot already contains in the same transaction; updates after the
//! session's last checkpoint are never pruned, so the checkpoint author,
//! level and actor are always read from the complete uncheckpointed log.
//! [`DocumentCollabRepository::seed_snapshot`] records the snapshot a session
//! is first built from, once, so every replica opens the same engine state;
//! [`DocumentCollabRepository::backlog`] reads, in one statement, the stored
//! snapshot a reader has fallen behind and the updates logged after what it
//! holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentHash, DocumentEditSessionId, UserId};

use super::Pools;
use super::collab_peers::{ActorColumns, UpdateAuthor, actor_from, level_str};
use super::rows::RepoResult;
use crate::documents::agents::DocumentActor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollabUpdateRow {
    pub seq: i64,
    pub update: Vec<u8>,
    pub author_user_id: UserId,
    pub actor: Option<DocumentActor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub hash: ContentHash,
    pub seq: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollabBacklog {
    pub snapshot: Option<StoredSnapshot>,
    pub rows: Vec<CollabUpdateRow>,
}

struct BacklogRow {
    snapshot_seq: i64,
    snapshot_sha256: Option<ContentHash>,
    seq: Option<i64>,
    update: Option<Vec<u8>>,
    author_user_id: Option<UserId>,
    actor_kind: Option<String>,
    actor_name: Option<String>,
    actor_attended: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct SnapshotMark<'a> {
    pub hash: &'a ContentHash,
    pub seq: i64,
    pub version: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct DocumentCollabRepository {
    pub(super) pools: Pools,
}

impl DocumentCollabRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn append(
        &self,
        session: &DocumentEditSessionId,
        update: &[u8],
        author: UpdateAuthor<'_>,
    ) -> RepoResult<Option<i64>> {
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
            return Ok(None);
        };
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
        Ok(Some(seq))
    }

    pub async fn list_after(
        &self,
        session: &DocumentEditSessionId,
        after_seq: i64,
    ) -> RepoResult<Vec<CollabUpdateRow>> {
        let rows = sqlx::query!(
            r#"SELECT seq, payload, author_user_id AS "author_user_id: UserId", actor_kind,
                      actor_name, actor_attended
               FROM document_collab_updates
               WHERE session_id = $1::text::uuid AND seq > $2
               ORDER BY seq"#,
            session.as_str(),
            after_seq,
        )
        .fetch_all(&*self.pools.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| CollabUpdateRow {
                seq: row.seq,
                update: row.payload,
                author_user_id: row.author_user_id,
                actor: actor_from(
                    row.actor_kind.as_deref(),
                    row.actor_name,
                    row.actor_attended,
                ),
            })
            .collect())
    }

    pub async fn backlog(
        &self,
        session: &DocumentEditSessionId,
        after_seq: i64,
    ) -> RepoResult<CollabBacklog> {
        let found = sqlx::query_as!(
            BacklogRow,
            r#"SELECT s.snapshot_seq AS "snapshot_seq!",
                      s.snapshot_sha256 AS "snapshot_sha256: ContentHash",
                      u.seq AS "seq?",
                      u.payload AS "update?",
                      u.author_user_id AS "author_user_id?: UserId",
                      u.actor_kind AS "actor_kind?",
                      u.actor_name AS "actor_name?",
                      u.actor_attended AS "actor_attended?"
               FROM document_edit_sessions s
               LEFT JOIN document_collab_updates u
                 ON u.session_id = s.id AND u.seq > GREATEST($2, s.snapshot_seq)
               WHERE s.id = $1::text::uuid
               ORDER BY u.seq"#,
            session.as_str(),
            after_seq,
        )
        .fetch_all(&*self.pools.write)
        .await?;
        let mut backlog = CollabBacklog::default();
        for row in found {
            if backlog.snapshot.is_none() && row.snapshot_seq > after_seq {
                backlog.snapshot = row.snapshot_sha256.clone().map(|hash| StoredSnapshot {
                    hash,
                    seq: row.snapshot_seq,
                });
            }
            if let (Some(seq), Some(update), Some(author_user_id)) =
                (row.seq, row.update, row.author_user_id)
            {
                backlog.rows.push(CollabUpdateRow {
                    seq,
                    update,
                    author_user_id,
                    actor: actor_from(
                        row.actor_kind.as_deref(),
                        row.actor_name,
                        row.actor_attended,
                    ),
                });
            }
        }
        Ok(backlog)
    }

    pub async fn stored_snapshot(
        &self,
        session: &DocumentEditSessionId,
    ) -> RepoResult<Option<StoredSnapshot>> {
        Ok(sqlx::query_as!(
            StoredSnapshot,
            r#"SELECT snapshot_sha256 AS "hash!: ContentHash", snapshot_seq AS seq
               FROM document_edit_sessions
               WHERE id = $1::text::uuid AND snapshot_sha256 IS NOT NULL"#,
            session.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn seed_snapshot(
        &self,
        session: &DocumentEditSessionId,
        hash: &ContentHash,
        version: &[u8],
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions
             SET snapshot_sha256 = $2, snapshot_vv = $3, updated_at = now()
             WHERE id = $1::text::uuid AND snapshot_sha256 IS NULL AND snapshot_seq = 0",
            session.as_str(),
            hash.as_str(),
            version,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn snapshot_seq(&self, session: &DocumentEditSessionId) -> RepoResult<Option<i64>> {
        Ok(sqlx::query_scalar!(
            "SELECT snapshot_seq FROM document_edit_sessions WHERE id = $1::text::uuid",
            session.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn set_snapshot(
        &self,
        session: &DocumentEditSessionId,
        snapshot: SnapshotMark<'_>,
    ) -> RepoResult<bool> {
        let SnapshotMark { hash, seq, version } = snapshot;
        let mut tx = self.pools.write.begin().await?;
        let moved = sqlx::query!(
            "UPDATE document_edit_sessions
             SET snapshot_sha256 = $2, snapshot_seq = $3, snapshot_vv = $4, updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL AND snapshot_seq < $3",
            session.as_str(),
            hash.as_str(),
            seq,
            version,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if moved {
            sqlx::query!(
                "DELETE FROM document_collab_updates u
                 USING document_edit_sessions s
                 WHERE u.session_id = $1::text::uuid AND s.id = u.session_id
                   AND u.seq <= LEAST($2, s.checkpoint_seq)",
                session.as_str(),
                seq,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(moved)
    }
}
