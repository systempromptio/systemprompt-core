use crate::error::RepositoryError;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiGatewayPolicyId, TenantId};

#[must_use]
#[derive(Debug, Clone)]
pub struct AiGatewayPolicyRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct GatewayPolicyRow {
    pub id: AiGatewayPolicyId,
    pub tenant_id: Option<String>,
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

    pub async fn find_for_tenant(
        &self,
        tenant_id: Option<&TenantId>,
    ) -> Result<Vec<GatewayPolicyRow>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id as "id!: AiGatewayPolicyId", tenant_id, name, spec, enabled
            FROM ai_gateway_policies
            WHERE enabled = TRUE
              AND (tenant_id = $1 OR tenant_id IS NULL)
            ORDER BY CASE WHEN tenant_id = $1 THEN 0 ELSE 1 END ASC, name ASC
            "#,
            tenant_id.map(TenantId::as_str)
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GatewayPolicyRow {
                id: r.id,
                tenant_id: r.tenant_id,
                name: r.name,
                spec: r.spec,
                enabled: r.enabled,
            })
            .collect())
    }

    pub async fn upsert(
        &self,
        tenant_id: Option<&TenantId>,
        name: &str,
        spec: &Value,
        enabled: bool,
    ) -> Result<AiGatewayPolicyId, RepositoryError> {
        let id = AiGatewayPolicyId::generate();
        let row = sqlx::query!(
            r#"
            INSERT INTO ai_gateway_policies (id, tenant_id, name, spec, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, name) DO UPDATE
            SET spec = EXCLUDED.spec,
                enabled = EXCLUDED.enabled,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id as "id!: AiGatewayPolicyId"
            "#,
            id.as_str(),
            tenant_id.map(TenantId::as_str),
            name,
            spec,
            enabled
        )
        .fetch_one(self.write_pool.as_ref())
        .await?;
        Ok(row.id)
    }
}
