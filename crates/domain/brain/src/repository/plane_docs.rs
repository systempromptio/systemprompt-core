//! The brain config documents (`filters`, `importance`, `retrieval`,
//! `synonyms`): one JSONB declaration per id, written by the config plane
//! inside the host's apply transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::plane::PlaneWrite;
use crate::models::PlaneRow;

#[derive(Debug, Clone)]
pub struct ConfigDocRepository {
    pools: Pools,
}

impl ConfigDocRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<PlaneRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            PlaneRow,
            "SELECT id, body, source, updated_at FROM brain_config_docs ORDER BY id"
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<PlaneRow>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        row: &PlaneWrite<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_config_docs (id, body, source) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET body = EXCLUDED.body, source = EXCLUDED.source, updated_at = now()",
            row.id,
            row.body,
            row.source.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete(conn: &mut PgConnection, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!("DELETE FROM brain_config_docs WHERE id = $1", id)
            .execute(conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
