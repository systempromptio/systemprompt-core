//! Entity-catalog persistence for access control.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use sqlx::PgConnection;

use super::AccessControlRepository;
use crate::authz::error::AuthzResult;
use crate::authz::types::{EntityKind, EntityRow};

impl AccessControlRepository {
    pub async fn get_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
    ) -> AuthzResult<Option<EntityRow>> {
        let row = sqlx::query_as!(
            EntityRow,
            r#"
            SELECT entity_type AS "kind: EntityKind", entity_id AS id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1 AND entity_id = $2
            "#,
            entity_type.as_str(),
            entity_id,
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_entities_bulk(
        &self,
        entity_type: EntityKind,
        entity_ids: &[String],
    ) -> AuthzResult<HashMap<String, EntityRow>> {
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query_as!(
            EntityRow,
            r#"
            SELECT entity_type AS "kind: EntityKind", entity_id AS id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1 AND entity_id = ANY($2)
            "#,
            entity_type.as_str(),
            entity_ids,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| (row.id.clone(), row)).collect())
    }

    pub async fn upsert_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
        default_included: bool,
        source: &str,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (entity_type, entity_id) DO UPDATE
            SET default_included = EXCLUDED.default_included,
                source = EXCLUDED.source,
                updated_at = NOW()
            "#,
            entity_type.as_str(),
            entity_id,
            default_included,
            source,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    // Why: leaves an existing row's `default_included` alone, so a write that
    // only needs the FK satisfied cannot widen access. `upsert_entity`
    // overwrites the flag; this does not.
    pub async fn ensure_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
        source: &str,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
            VALUES ($1, $2, false, $3)
            ON CONFLICT (entity_type, entity_id) DO NOTHING
            "#,
            entity_type.as_str(),
            entity_id,
            source,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    // Why: `access_control_rules` FKs onto this table ON DELETE CASCADE, so a
    // pruned entity takes its grants with it. Only call this for a kind whose
    // caller can enumerate the complete real set — an empty `keep` deletes
    // every row of the kind, which is why `gateway_entities` refuses one.
    pub async fn reconcile_entities(
        &self,
        entity_type: EntityKind,
        keep: &[&str],
        default_included: bool,
        source: &str,
    ) -> AuthzResult<u64> {
        let mut tx = self.write_pool.begin().await?;
        upsert_entities_on(&mut tx, entity_type, keep, default_included, source).await?;
        let keep_owned: Vec<String> = keep.iter().map(|id| (*id).to_owned()).collect();
        let res = sqlx::query!(
            r#"
            DELETE FROM access_control_entities
            WHERE entity_type = $1
              AND entity_id <> ALL($2::text[])
            "#,
            entity_type.as_str(),
            &keep_owned,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(res.rows_affected())
    }

    pub async fn list_entities(&self, entity_type: EntityKind) -> AuthzResult<Vec<EntityRow>> {
        let rows = sqlx::query_as!(
            EntityRow,
            r#"
            SELECT entity_type AS "kind: EntityKind", entity_id AS id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1
            ORDER BY entity_id
            "#,
            entity_type.as_str(),
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }
}

async fn upsert_entities_on(
    conn: &mut PgConnection,
    entity_type: EntityKind,
    ids: &[&str],
    default_included: bool,
    source: &str,
) -> AuthzResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let ids_owned: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    sqlx::query!(
        r#"
        INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
        SELECT $1, id, $3, $4
        FROM UNNEST($2::text[]) AS id
        ON CONFLICT (entity_type, entity_id) DO UPDATE
        SET default_included = EXCLUDED.default_included,
            source = EXCLUDED.source,
            updated_at = NOW()
        "#,
        entity_type.as_str(),
        &ids_owned,
        default_included,
        source,
    )
    .execute(conn)
    .await?;
    Ok(())
}
