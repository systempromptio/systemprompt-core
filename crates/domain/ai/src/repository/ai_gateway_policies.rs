//! Repository for `ai_gateway_policies` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiGatewayPolicyId;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiGatewayPolicyRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct GatewayPolicyRow {
    pub id: AiGatewayPolicyId,
    pub name: String,
    pub spec: Value,
    pub enabled: bool,
}

impl AiGatewayPolicyRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let pool = db
            .pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }

    pub async fn list_for_global(&self) -> Result<Vec<GatewayPolicyRow>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id as "id!: AiGatewayPolicyId", name, spec, enabled
            FROM ai_gateway_policies
            WHERE enabled = TRUE
            ORDER BY name ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GatewayPolicyRow {
                id: r.id,
                name: r.name,
                spec: r.spec,
                enabled: r.enabled,
            })
            .collect())
    }

    pub async fn upsert(
        &self,
        name: &str,
        spec: &Value,
        enabled: bool,
    ) -> Result<AiGatewayPolicyId, RepositoryError> {
        let id = AiGatewayPolicyId::generate();
        let row = sqlx::query!(
            r#"
            INSERT INTO ai_gateway_policies (id, name, spec, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (name) DO UPDATE
            SET spec = EXCLUDED.spec,
                enabled = EXCLUDED.enabled,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id as "id!: AiGatewayPolicyId"
            "#,
            id.as_str(),
            name,
            spec,
            enabled
        )
        .fetch_one(self.write_pool.as_ref())
        .await?;
        Ok(row.id)
    }

    /// Every policy name, including disabled ones — orphan detection and
    /// insert-vs-update both need the full set, not just the enabled rows.
    pub async fn list_all_names(&self) -> Result<Vec<String>, RepositoryError> {
        let names: Vec<String> = sqlx::query_scalar!("SELECT name FROM ai_gateway_policies")
            .fetch_all(self.pool.as_ref())
            .await?;
        Ok(names)
    }

    pub async fn delete_by_name(&self, name: &str) -> Result<(), RepositoryError> {
        sqlx::query!("DELETE FROM ai_gateway_policies WHERE name = $1", name)
            .execute(self.write_pool.as_ref())
            .await?;
        Ok(())
    }
}
