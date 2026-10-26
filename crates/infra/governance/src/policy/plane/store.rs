//! `governance_config` reads and writes: the plane's on the applier's
//! transaction, the engine's on the read pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sqlx::{PgConnection, PgPool};
use systemprompt_traits::RepositoryError;

pub async fn list_governance_config(
    pool: &PgPool,
) -> Result<BTreeMap<String, String>, RepositoryError> {
    let rows = sqlx::query!("SELECT entity, value FROM governance_config")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| (r.entity, r.value)).collect())
}

pub(super) async fn list_on(
    conn: &mut PgConnection,
) -> Result<BTreeMap<String, String>, RepositoryError> {
    let rows = sqlx::query!("SELECT entity, value FROM governance_config")
        .fetch_all(conn)
        .await?;
    Ok(rows.into_iter().map(|r| (r.entity, r.value)).collect())
}

pub(super) async fn upsert_on(
    conn: &mut PgConnection,
    entity: &str,
    value: &str,
    source: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r"INSERT INTO governance_config (entity, value, source)
          VALUES ($1, $2, $3)
          ON CONFLICT (entity) DO UPDATE
             SET value = EXCLUDED.value, source = EXCLUDED.source, updated_at = NOW()",
        entity,
        value,
        source,
    )
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn delete_on(
    conn: &mut PgConnection,
    entity: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!("DELETE FROM governance_config WHERE entity = $1", entity)
        .execute(conn)
        .await?;
    Ok(())
}
