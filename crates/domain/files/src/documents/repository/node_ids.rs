//! [`DocumentNodeIdRepository`]: the stable node ids of each revision, keyed
//! by the codec path they stand for, and the per-writer counters ids are
//! minted from.
//!
//! A revision's ids are written once, in one statement, inside the
//! transaction that writes the revision; a second write of the same
//! revision's ids is an error. Counters are reserved in blocks on the
//! primary (`document_node_counters`), so two commits by one writer never
//! mint the same id however far a replica lags. [`Self::without_ids`] pages
//! the revisions that have no ids, oldest first, for the backfill job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::NodeIdEntry;

#[derive(Debug, Clone)]
pub struct DocumentNodeIdRepository {
    pools: Pools,
}

impl DocumentNodeIdRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn list(&self, revision: &DocumentRevisionId) -> RepoResult<Vec<NodeIdEntry>> {
        Ok(sqlx::query_as!(
            NodeIdEntry,
            r#"SELECT node_id AS "node_id: DocumentNodeId",
                      path AS "path: DocumentNodeId",
                      hash8
               FROM document_node_ids
               WHERE revision_id = $1::text::uuid
               ORDER BY ordinal"#,
            revision.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn insert(
        conn: &mut PgConnection,
        document: &DocumentId,
        revision: &DocumentRevisionId,
        entries: &[NodeIdEntry],
    ) -> RepoResult<()> {
        let node_ids: Vec<String> = entries.iter().map(|e| e.node_id.to_string()).collect();
        let paths: Vec<String> = entries.iter().map(|e| e.path.to_string()).collect();
        let hashes: Vec<String> = entries.iter().map(|e| e.hash8.clone()).collect();
        let ordinals: Vec<i32> = (0..entries.len())
            .map(|i| i32::try_from(i).unwrap_or(i32::MAX))
            .collect();
        sqlx::query!(
            r#"INSERT INTO document_node_ids
                   (document_id, revision_id, node_id, path, hash8, ordinal)
               SELECT $1::text::uuid, $2::text::uuid, n, p, h, o
               FROM UNNEST($3::text[], $4::text[], $5::text[], $6::int4[]) AS t(n, p, h, o)"#,
            document.as_str(),
            revision.as_str(),
            &node_ids,
            &paths,
            &hashes,
            &ordinals,
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn reserve(
        conn: &mut PgConnection,
        document: &DocumentId,
        writer: &str,
        count: i64,
    ) -> RepoResult<i64> {
        let count = count.max(1);
        Ok(sqlx::query_scalar!(
            r#"INSERT INTO document_node_counters (document_id, writer, next)
               SELECT $1::text::uuid, $2,
                      COALESCE((SELECT MAX(split_part(node_id, '.', 2)::bigint)
                                FROM document_node_ids
                                WHERE document_id = $1::text::uuid
                                  AND split_part(node_id, '.', 1) = $2
                                  AND split_part(node_id, '.', 2) ~ '^[0-9]{1,18}$'), 0) + 1 + $3
               ON CONFLICT (document_id, writer)
                   DO UPDATE SET next = document_node_counters.next + $3
               RETURNING next - $3 AS "first!""#,
            document.as_str(),
            writer,
            count,
        )
        .fetch_one(conn)
        .await?)
    }

    pub async fn lock_revision(
        conn: &mut PgConnection,
        revision: &DocumentRevisionId,
    ) -> RepoResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM document_node_ids
                              WHERE revision_id = $1::text::uuid) AS "stored!"
               FROM document_revisions WHERE id = $1::text::uuid FOR UPDATE"#,
            revision.as_str(),
        )
        .fetch_optional(conn)
        .await?
        .unwrap_or(false))
    }

    pub async fn without_ids(
        &self,
        after: Option<(DateTime<Utc>, &DocumentRevisionId)>,
        limit: i64,
    ) -> RepoResult<Vec<(DateTime<Utc>, DocumentRevisionId)>> {
        let (after_at, after_id) =
            after.map_or((None, None), |(at, id)| (Some(at), Some(id.as_str())));
        Ok(sqlx::query!(
            r#"SELECT r.created_at, r.id::text AS "id!: DocumentRevisionId"
               FROM document_revisions r
               WHERE NOT EXISTS (SELECT 1 FROM document_node_ids n WHERE n.revision_id = r.id)
                 AND ($1::timestamptz IS NULL
                      OR (r.created_at, r.id) > ($1::timestamptz, $2::text::uuid))
               ORDER BY r.created_at, r.id
               LIMIT $3"#,
            after_at,
            after_id,
            limit,
        )
        .fetch_all(&*self.pools.write)
        .await?
        .into_iter()
        .map(|row| (row.created_at, row.id))
        .collect())
    }
}
