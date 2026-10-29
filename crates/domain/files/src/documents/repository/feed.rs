//! The bridge Drive change feed's reads over landed revisions.
//!
//! Every change carries a landing position: the id of the transaction that
//! landed it and a value from the `document_landing_seq` sequence shared
//! with the path history. A page only returns positions whose transaction
//! is older than the horizon, so a commit that lands later always sorts
//! after every position already served.
//!
//! The horizon is the oldest in-flight transaction that can still land a
//! position, or the next transaction id when there is none. A transaction
//! can land a position only while it holds `ROW EXCLUSIVE` on
//! `document_revisions` or `document_path_changes`, and it takes that lock
//! before it is assigned a transaction id: a single statement locks every
//! table it names before it writes, and a landing transaction locks
//! `document_revisions` as its first statement. The horizon is read as a
//! snapshot first and the lock table second, so an in-flight transaction the
//! snapshot lists is matched against its lock, and one with no id yet is
//! assigned an id at or past the snapshot's bound. Transactions that never
//! touch those tables, however long they stay open, do not hold the
//! horizon back. The horizon and the pages read the write pool: a replica
//! can lag the horizon it would report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentId, DocumentRevisionId, SessionId, TaskId, TraceId, UserId,
};

use super::revisions::{DocumentRevisionRepository, RevisionRow};
use super::rows::RepoResult;
use crate::documents::models::{DocumentRevision, DriveCursor};

/// A feed page bound: positions at or after this transaction id are
/// withheld because a transaction below it may still land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedHorizon(pub i64);

struct LandedRow {
    id: DocumentRevisionId,
    document_id: DocumentId,
    parent_revision_id: Option<DocumentRevisionId>,
    content_sha256: ContentHash,
    author_user_id: UserId,
    actor_kind: String,
    origin_kind: String,
    origin_id: Option<String>,
    message: Option<String>,
    // JSON: the JSONB edit_ops column, decoded into EditOp values on read.
    edit_ops: Option<serde_json::Value>,
    session_id: Option<SessionId>,
    context_id: Option<ContextId>,
    trace_id: Option<TraceId>,
    task_id: Option<TaskId>,
    on_behalf_of: Option<UserId>,
    created_at: DateTime<Utc>,
    landed_at: Option<DateTime<Utc>>,
    landing_xid: i64,
    landing_seq: i64,
}

impl LandedRow {
    fn split(self) -> RepoResult<(DocumentRevision, DriveCursor)> {
        let position = DriveCursor {
            xid: self.landing_xid,
            seq: self.landing_seq,
        };
        let revision = DocumentRevision::try_from(RevisionRow {
            id: self.id,
            document_id: self.document_id,
            parent_revision_id: self.parent_revision_id,
            content_sha256: self.content_sha256,
            author_user_id: self.author_user_id,
            actor_kind: self.actor_kind,
            origin_kind: self.origin_kind,
            origin_id: self.origin_id,
            message: self.message,
            edit_ops: self.edit_ops,
            session_id: self.session_id,
            context_id: self.context_id,
            trace_id: self.trace_id,
            task_id: self.task_id,
            on_behalf_of: self.on_behalf_of,
            created_at: self.created_at,
            landed_at: self.landed_at,
        })?;
        Ok((revision, position))
    }
}

impl DocumentRevisionRepository {
    pub async fn feed_horizon(&self) -> RepoResult<FeedHorizon> {
        let snapshot = sqlx::query_scalar!(r#"SELECT pg_current_snapshot()::text AS "snapshot!""#)
            .fetch_one(&*self.pools.write)
            .await?;
        let horizon = sqlx::query_scalar!(
            r#"SELECT LEAST(
                   pg_snapshot_xmax($1::text::pg_snapshot)::text::bigint,
                   (SELECT min(x::text::bigint)
                    FROM pg_snapshot_xip($1::text::pg_snapshot) AS x
                    WHERE x::text::bigint % 4294967296 IN (
                        SELECT own.transactionid::text::bigint
                        FROM pg_locks own
                        JOIN pg_locks rel ON rel.pid = own.pid
                        WHERE own.locktype = 'transactionid'
                          AND own.mode = 'ExclusiveLock'
                          AND own.granted
                          AND rel.locktype = 'relation'
                          AND rel.mode = 'RowExclusiveLock'
                          AND rel.database = (SELECT oid FROM pg_database
                                              WHERE datname = current_database())
                          AND rel.relation IN ('document_revisions'::regclass,
                                               'document_path_changes'::regclass)))
               ) AS "horizon!""#,
            snapshot,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(FeedHorizon(horizon))
    }

    pub async fn landed_since(
        &self,
        after: DriveCursor,
        horizon: FeedHorizon,
        limit: i64,
    ) -> RepoResult<Vec<(DocumentRevision, DriveCursor)>> {
        let rows = sqlx::query_as!(
            LandedRow,
            r#"SELECT id::text AS "id!: DocumentRevisionId",
                      document_id::text AS "document_id!: DocumentId",
                      parent_revision_id::text AS "parent_revision_id: DocumentRevisionId",
                      content_sha256 AS "content_sha256: ContentHash",
                      author_user_id AS "author_user_id: UserId",
                      actor_kind, origin_kind, origin_id, message, edit_ops,
                      session_id AS "session_id: SessionId",
                      context_id AS "context_id: ContextId",
                      trace_id AS "trace_id: TraceId",
                      task_id AS "task_id: TaskId",
                      on_behalf_of AS "on_behalf_of: UserId",
                      created_at, landed_at,
                      landing_xid AS "landing_xid!", landing_seq AS "landing_seq!"
               FROM document_revisions
               WHERE landing_xid IS NOT NULL AND landed_at IS NOT NULL
                 AND landing_xid < $3
                 AND (landing_xid, landing_seq) > ($1, $2)
               ORDER BY landing_xid, landing_seq
               LIMIT $4"#,
            after.xid,
            after.seq,
            horizon.0,
            limit.max(1),
        )
        .fetch_all(&*self.pools.write)
        .await?;
        rows.into_iter().map(LandedRow::split).collect()
    }
}
