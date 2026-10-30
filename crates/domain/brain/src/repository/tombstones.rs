//! [`TombstoneRepository`]: the identities of erased facts, consulted
//! before a candidate is stored so an erased fact is never re-extracted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{FactKindId, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;

#[derive(Debug, Clone)]
pub struct TombstoneRepository {
    pools: Pools,
}

impl TombstoneRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn exists(
        executor: impl PgExecutor<'_>,
        kind: &FactKindId,
        identity_hash: &str,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM brain_tombstones
                              WHERE kind = $1 AND identity_hash = $2) AS "exists!""#,
            kind.as_str(),
            identity_hash,
        )
        .fetch_one(executor)
        .await?)
    }

    pub async fn insert(
        conn: &mut PgConnection,
        kind: &FactKindId,
        identity_hash: &str,
        erased_by: Option<&UserId>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_tombstones (kind, identity_hash, erased_by) VALUES ($1, $2, $3)
             ON CONFLICT (kind, identity_hash) DO NOTHING",
            kind.as_str(),
            identity_hash,
            erased_by.map(UserId::as_str),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn is_tombstoned(
        &self,
        kind: &FactKindId,
        identity_hash: &str,
    ) -> Result<bool, RepositoryError> {
        Self::exists(&*self.pools.read, kind, identity_hash).await
    }
}
