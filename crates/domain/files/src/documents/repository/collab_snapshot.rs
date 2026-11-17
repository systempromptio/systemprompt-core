//! Moving an edit session's stored snapshot forward and pruning the updates
//! it now covers, in one transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentEditSessionId;

use super::collab_updates::{DocumentCollabRepository, SnapshotMark};
use super::rows::RepoResult;
use crate::documents::transient::retry_conflicts;

impl DocumentCollabRepository {
    pub async fn set_snapshot(
        &self,
        session: &DocumentEditSessionId,
        snapshot: SnapshotMark<'_>,
    ) -> RepoResult<bool> {
        retry_conflicts(|| self.set_snapshot_once(session, snapshot)).await
    }

    async fn set_snapshot_once(
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
