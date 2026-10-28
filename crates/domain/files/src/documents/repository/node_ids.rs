//! [`DocumentNodeIdRepository`]: the stable node ids of each revision, keyed
//! by the codec path they stand for.
//!
//! A revision's ids are written once, in one statement; a second writer of
//! the same revision (two readers minting a legacy revision at once) loses
//! on the `(revision_id, path)` key and reads the winner's ids back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        .fetch_all(&*self.pools.read)
        .await?)
    }

    pub async fn insert(
        &self,
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
               FROM UNNEST($3::text[], $4::text[], $5::text[], $6::int4[]) AS t(n, p, h, o)
               ON CONFLICT DO NOTHING"#,
            document.as_str(),
            revision.as_str(),
            &node_ids,
            &paths,
            &hashes,
            &ordinals,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn max_counter(&self, document: &DocumentId, writer: &str) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(split_part(node_id, '.', 2)::bigint), 0) AS "max!"
               FROM document_node_ids
               WHERE document_id = $1::text::uuid
                 AND split_part(node_id, '.', 1) = $2
                 AND split_part(node_id, '.', 2) ~ '^[0-9]{1,18}$'"#,
            document.as_str(),
            writer,
        )
        .fetch_one(&*self.pools.read)
        .await?)
    }
}
