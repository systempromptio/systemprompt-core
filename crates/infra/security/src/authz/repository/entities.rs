use std::str::FromStr;

use super::AccessControlRepository;
use crate::authz::error::AuthzResult;
use crate::authz::types::{EntityKind, EntityRow};

impl AccessControlRepository {
    /// `Ok(None)` means the entity has no catalog row at all (publish-pipeline
    /// bootstrap gap) — the resolver turns this into
    /// [`crate::authz::DenyReason::UnknownEntity`].
    pub async fn get_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
    ) -> AuthzResult<Option<EntityRow>> {
        let row = sqlx::query!(
            r#"
            SELECT entity_type, entity_id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1 AND entity_id = $2
            "#,
            entity_type.as_str(),
            entity_id,
        )
        .fetch_optional(&*self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(EntityRow {
            kind: EntityKind::from_str(&row.entity_type)?,
            id: row.entity_id,
            default_included: row.default_included,
            source: row.source,
        }))
    }

    /// Overwrites `default_included` and `source` on conflict so the most recent
    /// bootstrap pass wins — the publish pipeline is the source of truth and runs
    /// ahead of YAML grant ingestion.
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

    /// One statement for the whole batch, instead of `ids.len()` awaits of
    /// [`Self::upsert_entity`]; all rows share one `default_included` and
    /// `source`.
    pub async fn upsert_entities(
        &self,
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
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_entities(&self, entity_type: EntityKind) -> AuthzResult<Vec<EntityRow>> {
        let rows = sqlx::query!(
            r#"
            SELECT entity_type, entity_id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1
            ORDER BY entity_id
            "#,
            entity_type.as_str(),
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(EntityRow {
                kind: EntityKind::from_str(&row.entity_type)?,
                id: row.entity_id,
                default_included: row.default_included,
                source: row.source,
            });
        }
        Ok(out)
    }
}
